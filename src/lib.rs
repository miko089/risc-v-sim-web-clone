use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{Multipart, multipart::Field},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use std::path::Path;
use std::time::Duration;
use tempfile::{TempDir, tempdir};
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{error, info};

pub async fn health_handler() -> &'static str {
    "Ok"
}

pub async fn compile_s_to_elf(
    s_content: &[u8],
    as_binary: impl AsRef<Path>,
    ld_binary: impl AsRef<Path>,
) -> Result<TempDir> {
    let dir = tempdir()?;
    let s_path = dir.path().join("input.s");
    let o_path = dir.path().join("output.o");
    let elf_path = dir.path().join("output.elf");

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
    Ok(dir)
}

pub async fn run_simulator(
    dir_with_elf: TempDir,
    ticks: u32,
    simulator_binary: impl AsRef<Path>,
) -> Result<String> {
    let elf_path = dir_with_elf.path().join("output.elf");
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

pub async fn submit_handler(multipart: Multipart) -> (StatusCode, Json<serde_json::Value>) {
    let as_binary = std::env::var("AS_BINARY").unwrap_or_else(|_| "riscv64-elf-as".to_string());
    let ld_binary = std::env::var("LD_BINARY").unwrap_or_else(|_| "riscv64-elf-ld".to_string());
    let simulator_binary =
        std::env::var("SIMULATOR_BINARY").unwrap_or_else(|_| "simulator".to_string());

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

    let elf_content = match timeout(
        Duration::from_secs(5),
        compile_s_to_elf(&file_content, &as_binary, &ld_binary),
    )
    .await
    {
        Ok(Ok(elf)) => elf,
        Ok(Err(compilation_error)) => {
            error!("Compilation failed: {compilation_error:#}");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("{compilation_error:#}"),
                })),
            );
        }
        Err(_) => {
            error!("Compilation timed out");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "compilation timed out",
                })),
            );
        }
    };

    let stdout = match timeout(
        Duration::from_secs(10),
        run_simulator(elf_content, ticks, &simulator_binary),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(sim_error)) => {
            error!("Simulation failed {sim_error:#}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": sim_error.to_string()
                })),
            );
        }
        Err(_) => {
            error!("Simulation timed out");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "simulation timed out",
                })),
            );
        }
    };

    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => (StatusCode::OK, Json(json)),
        Err(e) => {
            error!("Simulator printed malformed JSON: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Invalid JSON output from simulator"
                })),
            )
        }
    }
}

pub fn create_app() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/submit", post(submit_handler))
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
