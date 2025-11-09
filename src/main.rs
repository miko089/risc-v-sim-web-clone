use std::net::{Ipv4Addr, SocketAddrV4};

use tracing::{Level, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::INFO)
        .init();

    let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 3000);
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    info!("Listening on port {port}");

    risc_v_sim_web::run(
        tracing::info_span!("rvsim-web"),
        listener,
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
            submissions_folder: std::env::var("SUBMISSIONS_FOLDER")
                .unwrap_or_else(|_| "submission".to_string())
                .into(),
            ticks_max: std::env::var("TICKS_MAX")
                .unwrap_or_else(|_| "150".to_string())
                .parse()
                .unwrap_or_else(|x| {
                    info!("can't parse {x} as a number, using 150");
                    150
                }),
            codesize_max: std::env::var("CODESIZE_MAX")
                .unwrap_or_else(|_| "250".to_string())
                .parse()
                .unwrap_or_else(|x| {
                    info!("can't parse {x} as a number, using 250");
                    250
                }),
        },
    )
    .await;
}
