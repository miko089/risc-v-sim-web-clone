pub mod auth;
pub mod database;
pub mod submission_actor;

use anyhow::{Context, Result, bail};
use axum::{
    Extension, Router,
    body::Body,
    extract::{Multipart, Query, State, multipart::Field},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self},
    response::Json,
    routing::{get, post},
};
use bytes;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::Deserialize;
use serde_json::json;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::{fs, net::TcpListener};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{Instrument, debug, error, info_span};
use ulid::Ulid;

use auth::{AuthState, Claims, auth_middleware};
use submission_actor::{
    Config as ActorConfig, SubmissionTask, run_submission_actor, submission_file,
};

pub struct Config {
    pub actor_config: ActorConfig,
    pub auth_state: AuthState,
}

#[derive(Deserialize)]
pub struct Submission {
    ulid: Ulid,
}

pub async fn health_handler() -> &'static str {
    "Ok"
}

fn extract_user_from_request(
    headers: &HeaderMap,
    auth_state: &AuthState,
) -> Result<(i64, String, Option<String>), StatusCode> {
    let auth_header = headers
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .split("jwt=")
        .nth(1)
        .and_then(|s| s.split(';').next())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(auth_state.jwt_secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claims = token_data.claims;
    let user_id = claims.sub.parse().map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok((user_id, claims.login, claims.name))
}

pub async fn parse_submit_inputs(
    mut multipart: Multipart,
    config: &Config,
) -> Result<(u32, bytes::Bytes)> {
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
    if ticks > config.actor_config.ticks_max {
        bail!("ticks number exceeds {}", config.actor_config.ticks_max)
    }
    if file.len() > config.actor_config.codesize_max as usize {
        bail!("file length exceeds {}", config.actor_config.codesize_max)
    }
    Ok((ticks, file))
}

async fn ticks_from_field(field: Field<'_>) -> Result<u32> {
    let ticks_str = field.text().await?;
    Ok(ticks_str.parse()?)
}

async fn submit_handler(
    State(config): State<Arc<Config>>,
    Extension(task_send): Extension<Sender<SubmissionTask>>,
    headers: HeaderMap,
    multipart: Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    // Extract user information from request
    let (user_id, user_login, user_name) =
        match extract_user_from_request(&headers, &config.auth_state) {
            Ok(user_info) => user_info,
            Err(e) => {
                debug!("Authentication failed: {:#?}", e);
                return (
                    e,
                    Json(serde_json::json!({
                        "error": "Authentication required"
                    })),
                );
            }
        };

    let (ticks, source_code) = match parse_submit_inputs(multipart, config.as_ref())
        .await
        .context("parse input")
    {
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
    debug!("Creating submission for user {} ({})", user_login, user_id);
    let send_res = task_send
        .send(SubmissionTask {
            source_code,
            ticks,
            ulid,
            user_id,
            user_login,
            user_name,
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
    let submission = submission_file(&config.actor_config, submission.ulid);
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

async fn user_submissions_handler(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    // Extract user information from request
    let (user_id, _user_login, _user_name) =
        match extract_user_from_request(&headers, &config.auth_state) {
            Ok(user_info) => user_info,
            Err(e) => {
                debug!("Authentication failed: {:#?}", e);
                return (
                    e,
                    Json(serde_json::json!({
                        "error": "Authentication required"
                    })),
                );
            }
        };

    match config
        .actor_config
        .db_service
        .get_user_submissions(user_id)
        .await
    {
        Ok(submissions) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "submissions": submissions
            })),
        ),
        Err(e) => {
            error!("Failed to fetch user submissions: {:#?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch submissions"
                })),
            )
        }
    }
}

pub async fn run(root_span: tracing::Span, listener: TcpListener, cfg: Config) {
    let (task_send, task_recv) = tokio::sync::mpsc::channel::<SubmissionTask>(100);
    let config = Arc::new(cfg);

    let submission_actor = run_submission_actor(Arc::new(config.actor_config.clone()), task_recv)
        .instrument(info_span!("submission_actor"));

    let router = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/health", get(health_handler))
                .route("/submit", post(submit_handler))
                .route("/submission", get(submission_handler))
                .route("/user-submissions", get(user_submissions_handler))
                .nest(
                    "/auth",
                    Router::new()
                        .route("/login", get(auth::login_handler))
                        .route("/callback", get(auth::callback_handler))
                        .route("/logout", get(auth::logout_handler))
                        .route("/me", get(auth::me_handler)),
                )
                .layer(Extension(task_send))
                .with_state(config.clone()),
        )
        .nest(
            "/auth",
            Router::new()
                .route("/login", get(auth::login_handler))
                .route("/callback", get(auth::callback_handler))
                .route("/logout", get(auth::logout_handler))
                .route("/me", get(auth::me_handler))
                .with_state(config.clone()),
        )
        .fallback_service(ServeDir::new("static"))
        .layer(ServiceBuilder::new().layer(tower_http::cors::CorsLayer::permissive()))
        .layer(middleware::from_fn_with_state(
            config.clone(),
            auth_middleware,
        ))
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
        );

    tokio::spawn(submission_actor);

    axum::serve(listener, router).await.unwrap();
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
