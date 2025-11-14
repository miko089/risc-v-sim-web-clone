mod submission_actor;

use anyhow::{Context, Result, bail};
use axum::{
    Extension, Router,
    body::Body,
    extract::{Multipart, Query, State, multipart::Field},
    http::{Request, StatusCode},
    response::Json,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::join;
use tokio::sync::mpsc::Sender;
use tokio::{fs, net::TcpListener};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{Instrument, debug, error, info_span};
use ulid::Ulid;

pub use submission_actor::Config;
use submission_actor::submission_file;

use crate::submission_actor::{SubmissionTask, run_submission_actor};

#[derive(Deserialize)]
pub struct Submission {
    ulid: Ulid,
}

pub async fn health_handler() -> &'static str {
    "Ok"
}

pub async fn parse_submit_inputs(mut multipart: Multipart) -> Result<(u32, bytes::Bytes)> {
    let mut ticks: Option<u32> = None;
    let mut file: Option<bytes::Bytes> = None;

    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name() else {
            bail!("field without name")
        };
        match name {
            "ticks" => ticks = Some(ticks_from_field(field).await.context("parsing ticks")?),
            "file" => file = Some(field.bytes().await.context("parsing file")?),
            name => bail!("unknown field {name:?}"),
        }
    }

    let Some(ticks) = ticks else {
        bail!("ticks field not set")
    };
    let Some(file) = file else {
        bail!("file field not set")
    };
    Ok((ticks, file))
}

async fn ticks_from_field(field: Field<'_>) -> Result<u32> {
    let ticks_str = field.text().await?;
    Ok(ticks_str.parse()?)
}

async fn submit_handler(
    Extension(task_send): Extension<Sender<SubmissionTask>>,
    multipart: Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    let (ticks, source_code) = match parse_submit_inputs(multipart).await.context("parse input") {
        Ok(x) => x,
        Err(e) => {
            debug!("Bad request: {e:#}");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("{e:#}"),
                })),
            );
        }
    };
    debug!(
        "Received {} bytes of program code to run for {ticks} ticks",
        source_code.len()
    );

    let ulid = Ulid::new();
    let send_res = task_send
        .send(SubmissionTask {
            source_code,
            ticks,
            ulid,
        })
        .await;
    if let Err(e) = send_res {
        error!("Failed to submit taks: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("{e}"),
            })),
        );
    }
    debug!("Submitted task with ulid {ulid}");

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "ulid": ulid
        })),
    )
}

async fn submission_handler(
    State(config): State<Arc<Config>>,
    submission: Query<Submission>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let submission = submission_file(&config, submission.ulid);
    let content = match fs::read(submission).await {
        Ok(x) => x,
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                return (StatusCode::NOT_FOUND, Json(serde_json::Value::Null));
            } else {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::Value::Null),
                );
            }
        }
    };
    let json_content = Json::from_bytes(&content);
    if let Err(e) = json_content {
        error!("{e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::Value::Null),
        );
    }
    (StatusCode::OK, json_content.unwrap())
}

pub async fn run(root_span: tracing::Span, listener: TcpListener, cfg: Config) {
    let (task_send, task_recv) = tokio::sync::mpsc::channel::<SubmissionTask>(100);
    let config = Arc::new(cfg);

    let submission_actor =
        run_submission_actor(config.clone(), task_recv).instrument(info_span!("submission_actor"));
    let router = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/health", get(health_handler))
                .route("/submit", post(submit_handler).layer(Extension(task_send)))
                .route("/submission", get(submission_handler)),
        )
        .fallback_service(ServeDir::new("static"))
        .layer(ServiceBuilder::new().layer(tower_http::cors::CorsLayer::permissive()))
        .layer(
            TraceLayer::new_for_http().make_span_with(move |request: &Request<Body>| {
                tracing::debug_span!(
                    parent: &root_span,
                    "request",
                    method = %request.method(),
                    uri = %request.uri(),
                    version = ?request.version(),
                )
            }),
        )
        .with_state(config.clone());

    let (res, _) = join!(axum::serve(listener, router), submission_actor,);
    res.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_handler() {
        let response = health_handler().await;
        assert_eq!(response, "Ok");
    }
}
