// WIP
#![allow(unused)]
use anyhow::anyhow;
use cardano_connector::CardanoConnector;
use cardano_sdk::{
    Address, Credential, Input, LeakableSigningKey, Output, Signature, SigningKey, Transaction,
    VerificationKey, address::kind, transaction::state::ReadyForSigning,
};
use std::{collections::BTreeMap, future::Future};

use crate::cardano::Cardano;

#[derive(Debug, Clone, clap::Args)]
pub struct WalletEnv {
    #[command(flatten)]
    pub cardano: super::cardano::CardanoEnv,

    #[arg(long, env = crate::meta::SIGNING_KEY)]
    pub signing_key: Option<LeakableSigningKey>,
}

impl WalletEnv {
    pub fn into_config(self) -> anyhow::Result<Config> {
        let cardano = self.cardano.into_config()?;
        let signing_key = self
            .signing_key
            .ok_or_else(|| anyhow!("signing key required, set SIGNING_KEY"))?
            .into_signing_key();
        Ok(Config {
            cardano,
            signing_key,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cardano: super::cardano::Config,
    pub signing_key: SigningKey,
}

impl Config {
    pub fn build(&self) -> Wallet {
        Wallet {
            cardano: self.cardano.build(),
            signing_key: self.signing_key.clone(),
        }
    }
}

pub struct Wallet {
    cardano: Cardano,
    signing_key: SigningKey,
}

impl Wallet {
    pub fn address(&self) -> Address<kind::Shelley> {
        self.verification_key()
            .to_address(self.cardano.network().into())
    }

    pub fn verification_key(&self) -> VerificationKey {
        self.signing_key.to_verification_key()
    }

    pub fn credential(&self) -> Credential {
        self.verification_key().to_credential()
    }

    pub fn utxos(&self) -> impl Future<Output = anyhow::Result<BTreeMap<Input, Output>>> + '_ {
        let credential = self.credential();
        async move { self.cardano.utxos_at(&credential, None).await }
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.signing_key.sign(msg)
    }

    pub fn submit<'a>(
        &'a self,
        tx: &'a Transaction<ReadyForSigning>,
    ) -> impl Future<Output = anyhow::Result<()>> + 'a {
        self.cardano.submit(tx)
    }
}
