mod auth;
mod errors;
mod native_logger;
mod secrets;
mod signing;
mod wallet;

pub use crate::auth::Auth;
pub use crate::errors::{LblError, LblRuntimeErrorCode};
pub use crate::native_logger::init_native_logger_once;
pub use crate::secrets::{
    derive_keys, generate_keypair, generate_mnemonic, Descriptors, KeyPair, WalletKeys,
};
pub use crate::signing::sign;
pub use crate::wallet::{AddressValidationResult, Config, Tx, TxDetails, TxStatus, Wallet};

pub use authors::{
    errors::{AuthError, AuthRuntimeErrorCode},
    AuthLevel,
};

use bdk::bitcoin::Network;
use bdk::Balance;
use log::Level as LogLevel;

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
