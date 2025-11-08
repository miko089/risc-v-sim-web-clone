mod common;
use common::*;

use tokio::{fs, task::JoinSet, time::Instant};
use ulid::Ulid;

use reqwest::{Client, Response};
use std::{path::Path, time::Duration};
use tracing::{Instrument, info, info_span};

const WAIT_TIMEOUT: f32 = 5.0;
const CONCURRENCY: usize = 5;

#[derive(serde::Deserialize)]
struct SubmitResponse {
    pub ulid: Ulid,
}

#[derive(serde::Deserialize)]
struct SubmissionResponse {
    pub ulid: Ulid,
    pub ticks: u32,
    pub code: String,
    #[allow(dead_code)]
    pub steps: serde_json::Value,
}

#[tokio::test]
async fn submit_simple() {
    run_test(
        "submit_simple",
        |_| {},
        async |port| {
            let client = reqwest::Client::new();
            let submit_response =
                submit_program(&client, port, 5, "riscv-samples/src/basic.s").await;
            let submit_status = submit_response.status();
            let resp_text = match submit_response.text().await {
                Ok(x) => format!("Response as text: {x}"),
                Err(e) => format!("Response has no text: {e}"),
            };
            assert_eq!(submit_status, reqwest::StatusCode::ACCEPTED, "{resp_text}");
        },
    )
    .await;
}

#[tokio::test]
async fn submit_and_wait() {
    run_test(
        "submit_and_wait",
        |_| {},
        async |port| {
            make_submission_and_wait_for_success(port, 5, "riscv-samples/src/basic.s").await;
        },
    )
    .await;
}

#[tokio::test]
async fn submit_non_existent() {
    run_test(
        "submit_non_existent",
        |_| {},
        async |port| {
            let client = reqwest::Client::new();
            let fake_submission_id = Ulid::new();
            let response = get_submission(&client, port, fake_submission_id).await;
            assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        },
    )
    .await;
}

#[tokio::test]
async fn submit_concurrent() {
    run_test(
        "submit_concurrent",
        |_| {},
        async |port| {
            let set = (0..CONCURRENCY)
                .map(|id| {
                    tokio::spawn(
                        make_submission_and_wait_for_success(port, 5, "riscv-samples/src/basic.s")
                            .instrument(info_span!("concurrent_client", id = id)),
                    )
                })
                .collect::<JoinSet<_>>();
            set.join_all().await;
        },
    )
    .await;
}

async fn make_submission_and_wait_for_success(port: u16, ticks: u32, path: impl AsRef<Path>) {
    let client = reqwest::Client::new();
    let original_code =
        String::from_utf8_lossy(&fs::read(path.as_ref()).await.unwrap()).to_string();
    let start = Instant::now();

    let submit_response = submit_program(&client, port, ticks, path.as_ref()).await;
    let submit_status = submit_response.status();
    assert_eq!(submit_status, reqwest::StatusCode::ACCEPTED);
    let submit_response = parse_response_json::<SubmitResponse>(submit_response).await;
    info!("Got submission id: {}", submit_response.ulid);

    let timeout = Duration::from_secs_f32(WAIT_TIMEOUT);
    let submission_response = tokio::time::timeout(
        timeout,
        wait_submission(&client, port, submit_response.ulid),
    )
    .await
    .unwrap();
    let dur = Instant::now().duration_since(start);
    info!("Waited for {:.2} seconds", dur.as_secs_f32());

    let submission_response = parse_response_json::<SubmissionResponse>(submission_response).await;
    assert_eq!(submission_response.ulid, submit_response.ulid);
    assert_eq!(submission_response.ticks, ticks);
    assert_eq!(submission_response.code, original_code);
}

async fn wait_submission(client: &Client, port: u16, submission_id: Ulid) -> Response {
    loop {
        let response = get_submission(&client, port, submission_id).await;
        match response.status() {
            reqwest::StatusCode::OK => (),
            reqwest::StatusCode::NOT_FOUND => {
                info!("Submission {submission_id} is not ready");
                tokio::time::sleep(Duration::from_secs_f32(0.5)).await;
                continue;
            }
            status => panic!("Unexpected HTTP status {status}"),
        }
        info!("Submission {submission_id} is ready");
        break response;
    }
}
