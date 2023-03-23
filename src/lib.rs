mod address;
mod auth;
mod errors;
mod native_logger;
mod secrets;
mod signing;
mod wallet;

pub use crate::address::AddressParsingError;
pub use crate::auth::Auth;
pub use crate::errors::{Error as WalletError, WalletRuntimeErrorCode};
pub use crate::native_logger::init_native_logger_once;
pub use crate::secrets::{
    derive_keys, generate_keypair, generate_mnemonic, words_by_prefix, Descriptors, KeyPair,
    WalletKeys,
};
pub use crate::signing::sign;
pub use crate::wallet::{Config, Tx, TxDetails, TxStatus, Wallet};

pub use honey_badger::graphql::errors::{
    Error as AuthError, GraphQlRuntimeErrorCode as AuthRuntimeErrorCode,
};
pub use honey_badger::AuthLevel;

use bdk::bitcoin::Network;
use bdk::Balance;
use log::Level as LogLevel;

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
