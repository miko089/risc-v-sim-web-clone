mod common;
use common::*;

#[tokio::test]
async fn submit_simple() {
    init_test();

    let port = 3000;
    let server_task = spawn_server(port).await;

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
    assert_eq!(submit_status, reqwest::StatusCode::OK, "{resp_text}");

    server_task.abort();
}
