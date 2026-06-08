mod cardano;
mod cmd;
mod json;
mod meta;
mod pretty;
mod wallet;
// mod config;
// mod connector;
// mod env;
// mod shared;
// mod tip;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cmd::Cmd::init()?.run().await
}
