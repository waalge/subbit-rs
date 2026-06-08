#[derive(Debug, clap::Args)]
pub struct Cmd {
    _unused: u32,
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        println!("Not yet impl");
        Ok(())
    }
}
