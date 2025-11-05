mod common;
use common::*;

use ulid::Ulid;

use reqwest::{Client, Response};
use std::time::Duration;
use tracing::info;

#[derive(serde::Deserialize)]
struct SubmitResponse {
    pub ulid: Ulid,
}

#[tokio::test]
async fn submit_simple() {
    run_test(
        "submit_simple",
        |_| {},
        async |port| {
            let client = reqwest::Client::new();
            let submit_response = submit_program(&client, port, 5, "samples/basic.s").await;
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
            let client = reqwest::Client::new();
            let submit_response = submit_program(&client, port, 5, "samples/basic.s").await;
            let submit_status = submit_response.status();
            assert_eq!(submit_status, reqwest::StatusCode::ACCEPTED);

            let response_bytes = submit_response.bytes().await.unwrap();
            let response = serde_json::from_slice::<SubmitResponse>(&response_bytes).unwrap();
            info!("Got submission id: {}", response.ulid);

            let submission_id = response.ulid;
            let timeout = Duration::from_secs_f32(5.0);
            tokio::time::timeout(timeout, wait_submission(&client, port, submission_id))
                .await
                .unwrap();
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
