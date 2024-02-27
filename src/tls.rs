use std::sync::Arc;

use anyhow::Result;
use hyper::server::conn::Http;
use rcgen::{BasicConstraints, IsCa};
use tokio::net::TcpListener;
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};
use tonic::transport::server::Routes;

pub fn generate_server_cert() -> Result<(Certificate, PrivateKey)> {
    let mut cp = rcgen::CertificateParams::new(vec!["localhost".to_string()]);
    cp.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let server_cert = rcgen::Certificate::from_params(cp)?;

    let cert = server_cert.serialize_pem()?;
    let certs: Vec<Certificate> = rustls_pemfile::certs(&mut cert.as_bytes())
        .into_iter()
        .map(|x| Certificate(x.unwrap().as_ref().to_vec()))
        .collect();

    let key: String = server_cert.serialize_private_key_pem();
    let key: PrivateKey = rustls_pemfile::pkcs8_private_keys(&mut key.as_bytes())
        .into_iter()
        .map(|x| PrivateKey(x.unwrap().secret_pkcs8_der().to_vec()))
        .next()
        .unwrap();

    Ok((certs[0].clone(), key))
}

pub async fn serve_with_tls(
    svc: Routes,
    certificate: Certificate,
    key: PrivateKey,
    addr: &str,
) -> Result<()> {
    let mut tls = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)?;
    tls.alpn_protocols = vec![b"h2".to_vec()];

    let mut http = Http::new();
    http.http2_only(true);

    let listener = TcpListener::bind(addr).await?;
    let tls_acceptor = TlsAcceptor::from(Arc::new(tls));

    loop {
        let (conn, _addr) = match listener.accept().await {
            Ok(incoming) => incoming,
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                continue;
            }
        };

        let http = http.clone();
        let tls_acceptor = tls_acceptor.clone();
        let svc = svc.clone();

        tokio::spawn(async move {
            let conn = tls_acceptor.accept(conn).await.unwrap();

            let svc = tower::ServiceBuilder::new().service(svc);

            http.serve_connection(conn, svc).await.unwrap();
        });
    }
}
