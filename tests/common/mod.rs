use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::task::JoinHandle;

use reqwest::Url;
use tracing::Level;

pub fn init_test() {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::TRACE)
        .init();
}

/// Spawns a risc-v-sim-web instance, listening on the specified port.
/// Make sure to .await the result of this function as soon as possible to
/// avoid any weird bugs.
/// The function returns a JoinHandle. For quick and clean test termination,
/// make sure to [`JoinHandle::abort()`] the returned future.
pub async fn spawn_server(port: u16) -> JoinHandle<()> {
    // NOTE: we specifically create a listener on the same thread and make the
    //       caller wait. This is because we want to make sure the server properly
    //       reserves the port. Otherwise the caller's HTTP requests will race
    //       and get a "connection refused" response.
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    tokio::spawn(async move {
        let app = risc_v_sim_web::create_app();
        axum::serve(listener, app).await.unwrap();
    })
}

/// Returns the server url.
pub fn server_url(port: u16) -> Url {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    Url::parse(&format!("http://{addr}")).unwrap()
}
