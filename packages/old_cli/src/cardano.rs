use anyhow::anyhow;
use cardano_connector_direct::Blockfrost;
use cardano_sdk::Network;

#[derive(Debug, Clone, clap::Args)]
pub struct CardanoEnv {
    #[arg(long, env = crate::meta::BLOCKFROST_PROJECT_ID)]
    pub blockfrost_project_id: Option<String>,

    #[arg(long, env = crate::meta::NETWORK, value_parser = parse_network)]
    pub network: Option<Network>,
}

fn parse_network(s: &str) -> Result<Network, String> {
    match s {
        "mainnet" => Ok(Network::Mainnet),
        "preprod" => Ok(Network::Preprod),
        "preview" => Ok(Network::Preview),
        _ => Err(format!("unknown network: {s}")),
    }
}

pub type Cardano = Blockfrost;

impl CardanoEnv {
    pub fn into_config(self) -> anyhow::Result<Config> {
        if let Some(project_id) = self.blockfrost_project_id {
            let network = self
                .network
                .or_else(|| network_from_project_id(&project_id))
                .ok_or_else(|| {
                    anyhow!("cannot deduce network from project id, provide --network")
                })?;
            Ok(Config::Blockfrost {
                project_id,
                network,
            })
        } else {
            Err(anyhow!(
                "no chain backend provided, set BLOCKFROST_PROJECT_ID"
            ))
        }
    }
}

fn network_from_project_id(project_id: &str) -> Option<Network> {
    if project_id.starts_with("mainnet") {
        Some(Network::Mainnet)
    } else if project_id.starts_with("preprod") {
        Some(Network::Preprod)
    } else if project_id.starts_with("preview") {
        Some(Network::Preview)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub enum Config {
    Blockfrost {
        project_id: String,
        network: Network,
    },
}

impl Config {
    #[allow(unused)]
    pub fn network(&self) -> Network {
        match self {
            Config::Blockfrost { network, .. } => *network,
        }
    }

    pub fn build(&self) -> Cardano {
        match self {
            Config::Blockfrost { project_id, .. } => Blockfrost::new(project_id.clone()),
        }
    }
}
