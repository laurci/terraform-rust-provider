mod cty;
mod server;
mod tls;
mod ubicloud;
mod util;

use std::{fs::File, sync::Mutex};

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD_NO_PAD as base64_engine, Engine as _};
use tonic::transport::Server;
use tracing::info;

use server::{tf::provider_server::ProviderServer, UbicloudProvider};
use tls::{generate_server_cert, serve_with_tls};

fn init_tracing() -> Result<()> {
    let log_file = File::create("ubicloud-trace.log")?;
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(Mutex::new(log_file))
        .with_ansi(false)
        .init();

    Ok(())
}

const PORT: u16 = 1100;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let provider = UbicloudProvider::new();

    let addr = format!("127.0.0.1:{PORT}");
    info!("prvovider listening on {}", addr);

    let (cert, key) = generate_server_cert()?;

    let serve = Server::builder()
        .add_service(ProviderServer::new(provider))
        .into_service();

    async fn handshake(server_cert: &[u8]) {
        let server_cert = base64_engine.encode(&server_cert);

        info!("1|6|tcp|localhost:{PORT}|grpc|{server_cert}");
        println!("1|6|tcp|localhost:{PORT}|grpc|{server_cert}");
    }

    tokio::join!(
        serve_with_tls(serve, cert.clone(), key, &addr),
        handshake(cert.as_ref())
    )
    .0?;

    Ok(())
}
