mod common;
use std::time::Duration;

use common::*;

use tokio::time::Instant;
use tracing::info;
use ulid::Ulid;

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
            let request_url = server_url(port).join("api/submit").unwrap();
            let client = reqwest::Client::new();
            let form = reqwest::multipart::Form::new()
                .text("ticks", 5.to_string())
                .file("file", "samples/basic.s")
                .await
                .unwrap();
            let submit_response = client
                .post(request_url)
                .multipart(form)
                .send()
                .await
                .unwrap();
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
            let request_url = server_url(port).join("api/submit").unwrap();
            let client = reqwest::Client::new();
            let form = reqwest::multipart::Form::new()
                .text("ticks", 5.to_string())
                .file("file", "samples/basic.s")
                .await
                .unwrap();
            let submit_response = client
                .post(request_url)
                .multipart(form)
                .send()
                .await
                .unwrap();
            let submit_status = submit_response.status();
            assert_eq!(submit_status, reqwest::StatusCode::ACCEPTED);

            let response_bytes = submit_response.bytes().await.unwrap();
            let response = serde_json::from_slice::<SubmitResponse>(&response_bytes).unwrap();
            info!("Got submission id: {}", response.ulid);

            let start = Instant::now();
            let submission_id = response.ulid;
            loop {
                if Instant::now().duration_since(start) >= Duration::from_secs_f32(5.0) {
                    panic!("The server took too long");
                }

                let request_url = server_url(port).join("api/submission").unwrap();
                let response = client
                    .get(request_url)
                    .query(&[("ulid", submission_id.to_string().as_str())])
                    .send()
                    .await
                    .unwrap();

                if response.status() != reqwest::StatusCode::OK
                    && response.status() != reqwest::StatusCode::NOT_FOUND
                {
                    panic!("Unexpected HTTP status {}", response.status());
                }

                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    info!("Submission {submission_id} is not ready");
                    tokio::time::sleep(Duration::from_secs_f32(0.5)).await;
                    continue;
                }
                info!("Submission {submission_id} is ready");
                break;
            }
        },
    )
    .await;
}
