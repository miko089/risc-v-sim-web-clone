mod common;
use common::*;

#[tokio::test]
async fn health_check() {
    let port = 3000;
    let server_task = spawn_server(port).await;

    let request_url = server_url(port).join("api/health").unwrap();
    let health_response = reqwest::get(request_url).await.unwrap();
    assert_eq!(health_response.status(), reqwest::StatusCode::OK);
    let health_response_text = health_response.text().await.unwrap();
    assert_eq!(health_response_text, "Ok");

    server_task.abort();
}
