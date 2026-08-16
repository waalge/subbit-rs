use clap::Args;
use pingora::prelude::*;
use std::{net::SocketAddr, time::Duration};
pub use subbit_client::{Client, HEADER, Outcome};

#[derive(Args, Debug)]
pub struct SubbitArgs {
    #[arg(long, env = "SUBBIT_SERVER")]
    subbit_server: Option<SocketAddr>,
    #[arg(long, env = "SUBBIT_TIMEOUT_MS", default_value = "200")]
    subbit_timeout_ms: u64,
}

impl SubbitArgs {
    pub fn into_config(self) -> Option<Client> {
        self.subbit_server.map(|addr| {
            tracing::info!(%addr, "subbit-server integration enabled");
            Client::new(addr, Duration::from_millis(self.subbit_timeout_ms))
                .expect("bad subbit client config")
        })
    }
}

/// Reads the `subbit` header + path off `session` and asks subbit-server.
pub async fn check(client: &Client, session: &Session) -> Outcome {
    let path = session.req_header().uri.path();
    let header = session
        .req_header()
        .headers
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    client.spend(path, header).await
}
