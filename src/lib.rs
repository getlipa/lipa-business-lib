mod errors;
mod native_logger;

use crate::errors::MnemonicGenerationError;
use crate::native_logger::init_native_logger_once;
use bdk::keys::bip39::Mnemonic;

use log::Level as LogLevel;
use rand::rngs::OsRng;
use rand::RngCore;

fn generate_random_bytes() -> Result<[u8; 32], MnemonicGenerationError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|e| MnemonicGenerationError::EntropyGeneration {
            message: e.to_string(),
        })?;
    Ok(bytes)
}

pub fn generate_mnemonic() -> Result<Vec<String>, MnemonicGenerationError> {
    let entropy = generate_random_bytes()?;
    let mnemonic = Mnemonic::from_entropy(&entropy).map_err(|e| {
        MnemonicGenerationError::MnemonicFromEntropy {
            message: e.to_string(),
        }
    })?;

    let mnemonic: Vec<String> = mnemonic.word_iter().map(|s| s.to_string()).collect();

    Ok(mnemonic)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_mnemonic_generation() {
        let mnemonic_string = generate_mnemonic().unwrap();
        assert_eq!(mnemonic_string.len(), 24);
    }

    #[test]
    fn test_mnemonic_code_decode() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();
        assert_eq!(mnemonic_string.join(" "), mnemonic.to_string());
    }
}

include!(concat!(env!("OUT_DIR"), "/lipabusinesslib.uniffi.rs"));
