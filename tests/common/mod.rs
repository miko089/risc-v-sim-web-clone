use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::task::JoinHandle;

use reqwest::Url;
use tracing::{Level, info};

pub fn init_test() {
    // Tests run in parallel, so some might have already created the logger.
    let _ = tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .try_init();
}

/// Spawns a risc-v-sim-web instance, listening on the specified port.
/// Make sure to .await the result of this function as soon as possible to
/// avoid any weird bugs.
/// The function returns a JoinHandle. For quick and clean test termination,
/// make sure to [`JoinHandle::abort()`] the returned future.
pub async fn spawn_server(cfg: risc_v_sim_web::Config) -> (u16, JoinHandle<()>) {
    // NOTE: we specifically create a listener on the same thread and make the
    //       caller wait. This is because we want to make sure the server properly
    //       reserves the port. Otherwise the caller's HTTP requests will race
    //       and get a "connection refused" response.
    let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    info!("Listening on port {port}");

    let task = tokio::spawn(risc_v_sim_web::run(listener, cfg));
    (port, task)
}

pub fn default_config(testname: &str) -> risc_v_sim_web::Config {
    risc_v_sim_web::Config {
        as_binary: std::env::var("AS_BINARY")
            .unwrap_or_else(|_| "riscv64-elf-as".to_string())
            .into(),
        ld_binary: std::env::var("LD_BINARY")
            .unwrap_or_else(|_| "riscv64-elf-ld".to_string())
            .into(),
        simulator_binary: std::env::var("SIMULATOR_BINARY")
            .unwrap_or_else(|_| "simulator".to_string())
            .into(),
        submissions_folder: format!("submissions-{testname}").into(),
    }
}

/// Returns the server url.
pub fn server_url(port: u16) -> Url {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    Url::parse(&format!("http://{addr}")).unwrap()
}
