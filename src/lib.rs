mod address;
mod errors;
mod native_logger;
mod secrets;
mod signing;
mod wallet;

pub use crate::address::AddressParsingError;
pub use crate::errors::{LipaError, RuntimeErrorCode};
pub use crate::native_logger::init_native_logger_once;
pub use crate::secrets::{
    derive_keys, generate_keypair, generate_mnemonic, Descriptors, KeyPair, WalletKeys,
};
pub use crate::signing::sign;
pub use crate::wallet::{Config, Tx, TxDetails, TxStatus, Wallet};

use bdk::bitcoin::Network;
use bdk::Balance;
use log::Level as LogLevel;

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
