extern crate core;

mod errors;
mod native_logger;
mod secrets;
mod signing;
mod wallet;

use crate::errors::{KeyDerivationError, KeyGenerationError, SigningError, WalletError};
use crate::native_logger::init_native_logger_once;
use crate::secrets::{
    derive_keys, generate_keypair, generate_mnemonic, Descriptors, KeyPair, LipaKeys,
};
use crate::signing::sign_message;
use crate::wallet::{Config, Wallet};

use bdk::bitcoin::Network;
use bdk::Balance;
use log::Level as LogLevel;

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
