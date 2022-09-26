mod errors;
mod native_logger;

use crate::errors::WalletGenerationError;
use crate::native_logger::init_native_logger_once;

use log::Level as LogLevel;

pub struct Wallet {
    private_key: Vec<u8>,
}

pub fn generate_wallet() -> Result<Wallet, WalletGenerationError> {
    Ok(Wallet {
        private_key: vec![1, 2, 3],
    })
}

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
