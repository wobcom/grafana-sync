pub mod peer;
pub mod network;
pub mod error;
pub mod message;
mod authentication;
mod encryption;

pub use peer::*;
pub use network::*;
pub use error::*;
pub use message::*;

pub const VERSION: u64 = 1;

const BINCONF: bincode::config::Configuration = bincode::config::standard();
