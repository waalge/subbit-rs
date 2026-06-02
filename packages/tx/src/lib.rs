mod network_parameters;
pub use network_parameters::NetworkParameters;

pub mod ops;

mod utxos;
pub use utxos::Utxos;

pub mod validator;
pub use validator::{MIN_ADA_BUFFER, VALIDATOR};

pub mod channel;
pub mod tx;

pub mod fuel;
pub mod step;

pub type Lovelace = u64;

pub const FEE_BUFFER: Lovelace = 3_000_000;
