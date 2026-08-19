use subbit_core::{
    Auth, Duration,
    base64::{from_base64, to_base64},
    envelope::{Body, Error as ServerError, Request, Response, Status},
};

use crate::{Account, Cache, Config, now};

#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    #[error("failed to decode response envelope")]
    Decode,
    #[error("server error: {0:?}")]
    Server(ServerError),
    #[error("iou signature did not verify")]
    BadSignature,
}

pub struct Issuer {
    config: Config,
    cache: Cache,
}

impl Issuer {
    pub fn new(account: Account, ttl_relative: Duration) -> Self {
        Self::from_parts(Config::new(account, ttl_relative), Cache::new())
    }

    /// Resume a session from persisted `Config`/`Cache`.
    pub fn from_parts(config: Config, cache: Cache) -> Self {
        Self { config, cache }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub fn spent(&self) -> u64 {
        self.cache.spent()
    }

    pub fn committed(&self) -> Option<u64> {
        self.cache.committed()
    }

    pub fn set_committed(&mut self, committed: u64) {
        self.cache.set_committed(committed);
    }

    fn now(&self) -> Duration {
        // FIXME:: Rationalize
        now().unwrap_or(Duration::from_secs(0))
    }

    fn auth(&self) -> Auth<Body, Body> {
        match self.cache.mac() {
            Some(mac) => Auth::Mac(mac.clone()),
            None => Auth::Pop(
                self.config
                    .account()
                    .pop(self.now() + self.config.ttl_relative()),
            ),
        }
    }

    /// Base64 request envelope committing `committed + cost`. Send it
    /// yourself (native reqwest, JS fetch, ...); hand the reply to `response`.
    pub fn spend(&mut self, cost: u64) -> String {
        let spent = self.spent();
        let required = spent + cost;
        let iou = if self.committed().unwrap_or(0) > required {
            None
        } else {
            let iou = self.config.account().iou(required);
            Some(iou)
        };
        self.cache.set_spent(required);
        to_base64(&Request {
            auth: self.auth(),
            iou,
        })
    }

    /// Parse, verify, and apply a base64 response envelope from the server.
    /// Commits from the verified iou's own amount — no local bookkeeping of
    /// what was requested needed.
    pub fn response(&mut self, envelope: &str) -> Result<(), ResponseError> {
        let status: Status = from_base64::<Response>(envelope)
            .map_err(|_| ResponseError::Decode)?
            .map_err(ResponseError::Server)?;

        if !self.config.account().verify_iou(&status.iou) {
            return Err(ResponseError::BadSignature);
        }

        self.cache.set_committed(status.iou.amount());
        self.cache
            .set_spent(status.spendable.unwrap_or(self.cache.spent()));
        if let Some(mac) = status.mac {
            self.cache.set_mac(mac);
        }
        Ok(())
    }
}
