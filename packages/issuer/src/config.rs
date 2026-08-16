use minicbor::{Decode, Encode};
use subbit_core::Duration;

use crate::Account;

#[derive(Encode, Decode)]
pub struct Config {
    #[n(0)]
    account: Account,
    #[n(1)]
    ttl_relative: Duration,
}

impl Config {
    pub fn new(account: Account, ttl_relative: Duration) -> Self {
        Self {
            account,
            ttl_relative,
        }
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn ttl_relative(&self) -> Duration {
        self.ttl_relative
    }
}
