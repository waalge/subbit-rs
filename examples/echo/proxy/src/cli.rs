use clap::Parser;
use pingora::prelude::*;
use std::net::SocketAddr;

use crate::proxy::EchoProxy;

#[cfg(feature = "subbit")]
use crate::subbit::SubbitArgs;

#[derive(Parser)]
pub struct Cli {
    #[arg(long, env = "ECHO_PROXY_LISTEN", default_value = "0.0.0.0:6432")]
    listen: SocketAddr,
    #[arg(long, env = "ECHO_PROXY_UPSTREAM", default_value = "127.0.0.1:3000")]
    upstream: SocketAddr,

    #[cfg(feature = "subbit")]
    #[command(flatten)]
    subbit: SubbitArgs,
}

impl Cli {
    pub fn run(self) {
        let mut server = Server::new(None).expect("failed to create server");
        server.bootstrap();

        #[cfg(feature = "subbit")]
        let proxy_impl = EchoProxy::new(self.upstream, self.subbit.into_config());

        #[cfg(not(feature = "subbit"))]
        let proxy_impl = EchoProxy::new(self.upstream);

        let mut proxy = http_proxy_service(&server.configuration, proxy_impl);
        proxy.add_tcp(&self.listen.to_string());
        server.add_service(proxy);

        tracing::info!(listen = %self.listen, upstream = %self.upstream, "echo-proxy listening");
        server.run_forever();
    }
}
