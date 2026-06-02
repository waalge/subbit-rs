use cardano_sdk::LeakableSigningKey;
use subbit_core::{Iou, Tag, Tbs};

/// Show
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Iou {
        /// Signing key (hex-encoded 32 bytes)
        #[clap(long, env = crate::meta::IOU_KEY)]
        iou_key: LeakableSigningKey,

        /// Channel tag (hex-encoded)
        #[clap(long)]
        tag: Tag,

        /// Amount
        #[clap(long)]
        amount: u64,
    },
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Cmd::Iou {
                iou_key,
                tag,
                amount,
            } => {
                let tbs = Tbs::new(tag, amount);
                let message = tbs.to_vec();
                let signature = iou_key.sign(&message);
                let iou = Iou::new(amount, <[u8; 64]>::from(signature).into());
                let cbor = minicbor::to_vec(&iou).unwrap();
                println!("{}", hex::encode(cbor));
                Ok(())
            }
        }
    }
}
