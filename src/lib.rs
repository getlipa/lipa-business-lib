mod errors;
mod native_logger;
mod secrets;
mod signing;

use crate::errors::{KeyDerivationError, MnemonicGenerationError, SigningError};
use crate::native_logger::init_native_logger_once;
use crate::secrets::{derive_keys, generate_mnemonic, KeyPair, LipaKeys};
use crate::signing::sign_message;

use bdk::bitcoin::Network;
use log::Level as LogLevel;

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
