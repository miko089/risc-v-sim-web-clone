use std::net::{Ipv4Addr, SocketAddrV4};

use risc_v_sim_web::create_app;
use tracing::{Level, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::INFO)
        .init();

    let app = create_app();

    let port = 3000;
    let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    info!("Listening on port {port}");

    axum::serve(listener, app).await.unwrap();
}
