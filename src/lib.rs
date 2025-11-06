use anyhow::{Context, Result, bail};
use axum::{
    Router,
    body::Body,
    extract::{Multipart, Query, State, multipart::Field},
    http::{Request, StatusCode},
    response::Json,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::{io::ErrorKind, time::Duration};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::process::Command;
use tokio::time::timeout;
use tokio::{fs, net::TcpListener};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{Instrument, error, info, info_span};
use ulid::{ULID_LEN, Ulid};

#[derive(Deserialize)]
pub struct Submission {
    ulid: Ulid,
}

pub async fn health_handler() -> &'static str {
    "Ok"
}

pub async fn compile_s_to_elf(
    s_content: &[u8],
    submission_dir: impl AsRef<Path>,
    as_binary: impl AsRef<Path>,
    ld_binary: impl AsRef<Path>,
) -> Result<()> {
    let dir = submission_dir.as_ref();
    let s_path = dir.join("input.s");
    let o_path = dir.join("output.o");
    let elf_path = dir.join("output.elf");

    info!("Writing program to {s_path:?}");
    fs::write(&s_path, s_content).await?;

    info!("Compiling {s_path:?} to object file {o_path:?}");
    let as_output = Command::new(as_binary.as_ref())
        .arg(&s_path)
        .arg("-o")
        .arg(&o_path)
        .kill_on_drop(true)
        .output()
        .await
        .context("copmiling")?;

    if !as_output.status.success() {
        let stderr = String::from_utf8_lossy(&as_output.stderr);
        let stdout = String::from_utf8_lossy(&as_output.stdout);
        return Err(anyhow::anyhow!("Assembler error:\n{}\n{}", stderr, stdout));
    }

    info!("Linking {o_path:?} to elf {elf_path:?}");
    let ld_output = Command::new(ld_binary.as_ref())
        .arg(&o_path)
        .arg("-Ttext=0x80000000")
        .arg("-o")
        .arg(&elf_path)
        .kill_on_drop(true)
        .output()
        .await
        .context("linking")?;

    if !ld_output.status.success() {
        let stderr = String::from_utf8_lossy(&ld_output.stderr);
        let stdout = String::from_utf8_lossy(&ld_output.stdout);
        return Err(anyhow::anyhow!("Linker error:\n{}\n{}", stderr, stdout));
    }

    info!("Elf ready");
    Ok(())
}

pub async fn run_simulator(
    dir_with_elf: impl AsRef<Path>,
    ticks: u32,
    simulator_binary: impl AsRef<Path>,
) -> Result<String> {
    let dir_with_elf = dir_with_elf.as_ref();
    let elf_path = dir_with_elf.join("output.elf");
    info!("Simulating the program at {elf_path:?}");

    let output = Command::new(simulator_binary.as_ref())
        .arg("--ticks")
        .arg(ticks.to_string())
        .arg("--path")
        .arg(&elf_path)
        .kill_on_drop(true)
        .output()
        .await
        .context("simulating")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // FIXME: This will dump simulator logs.
    //        Not something we actually want to do.
    if !output.status.success() {
        bail!("simulator failed: {stderr}");
    }

    info!("Simulating has been successful");
    Ok(stdout)
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

async fn simulate(
    file_content: bytes::Bytes,
    path: impl AsRef<Path>,
    as_binary: impl AsRef<Path>,
    ld_binary: impl AsRef<Path>,
    simulator_binary: impl AsRef<Path>,
    ticks: u32,
    ulid: Ulid,
) -> Json<serde_json::Value> {
    match timeout(
        Duration::from_secs(5),
        compile_s_to_elf(&file_content, &path, &as_binary, &ld_binary),
    )
    .await
    {
        Ok(Ok(elf)) => elf,
        Ok(Err(compilation_error)) => {
            error!("Compilation failed: {compilation_error:#}");
            return Json(serde_json::json!({
                    "error": format!("{compilation_error:#}"),
            }));
        }
        Err(_) => {
            error!("Compilation timed out");
            return Json(serde_json::json!({
                    "error": "compilation timed out",
            }));
        }
    };

    let stdout = match timeout(
        Duration::from_secs(10),
        run_simulator(&path, ticks, &simulator_binary),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(sim_error)) => {
            error!("Simulation failed {sim_error:#}");
            return Json(serde_json::json!({
                "error": sim_error.to_string()
            }));
        }
        Err(_) => {
            error!("Simulation timed out");
            return Json(serde_json::json!({
                "error": "simulation timed out",
            }));
        }
    };

    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(mut json) => {
            if let serde_json::Value::Object(ref mut map) = json {
                map.insert(
                    "ulid".to_string(),
                    serde_json::Value::String(ulid.to_string()),
                );
                map.insert(
                    "code".to_string(),
                    serde_json::Value::String(
                        str::from_utf8(&file_content)
                            .unwrap_or_else(|_| "")
                            .to_string(),
                    ),
                );
                map.insert("ticks".to_string(), json!(ticks));
            }
            return Json(json);
        }
        Err(e) => {
            error!("Simulator printed malformed JSON: {e}");
            return Json(serde_json::json!({
                "error": "Invalid JSON output from simulator"
            }));
        }
    }
}

async fn submit_handler(
    State(config): State<Arc<Config>>,
    multipart: Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    let ulid = Ulid::new();
    let mut buf = [0u8; ULID_LEN];
    let ulid_str = ulid.array_to_str(&mut buf);
    let submission_dir = config.submissions_folder.join(&ulid_str);

    if let Err(e) = fs::create_dir_all(&submission_dir).await {
        error!("can't create {:#?}: {e}", submission_dir);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json::from(serde_json::Value::Null),
        );
    }

    let (ticks, file_content) = match parse_submit_inputs(multipart).await.context("parse input") {
        Ok(x) => x,
        Err(e) => {
            info!("Bad request: {e:#}");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("{e:#}"),
                })),
            );
        }
    };
    info!(
        "Received {} bytes of program code to run for {ticks} ticks",
        file_content.len()
    );

    tokio::spawn(
        async move {
            let Json(json): Json<serde_json::Value> = simulate(
                file_content,
                &submission_dir,
                &config.as_binary,
                &config.ld_binary,
                &config.simulator_binary,
                ticks,
                ulid,
            )
            .await;
            if let Err(e) = fs::write(submission_dir.join("simulation.json"), json.to_string()).await {
                error!("couldn't write simulation.json at {submission_dir:?}: {e}");
            };
        }
        .instrument(info_span!(parent: tracing::Span::current(), "submit_task", ulid = %ulid)),
    );

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
    let mut buf = [0u8; ULID_LEN];
    let ulid_str = submission.ulid.array_to_str(&mut buf);
    let mut path = config.submissions_folder.clone();
    path.extend([&ulid_str, "simulation.json"]);
    let content = match fs::read(path).await {
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

pub struct Config {
    pub as_binary: PathBuf,
    pub ld_binary: PathBuf,
    pub simulator_binary: PathBuf,
    pub submissions_folder: PathBuf,
}

pub async fn run(root_span: tracing::Span, listener: TcpListener, cfg: Config) {
    let def_span = move |request: &Request<Body>| {
        tracing::debug_span!(
            parent: &root_span,
            "request",
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
        )
    };

    let state = Arc::new(cfg);
    let router = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/health", get(health_handler))
                .route("/submit", post(submit_handler))
                .route("/submission", get(submission_handler)),
        )
        .fallback_service(ServeDir::new("static"))
        .layer(ServiceBuilder::new().layer(tower_http::cors::CorsLayer::permissive()))
        .layer(TraceLayer::new_for_http().make_span_with(def_span))
        .with_state(state);

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
