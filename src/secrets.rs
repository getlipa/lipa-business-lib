use crate::errors::{KeyDerivationError, MnemonicGenerationError};
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey};
use bdk::bitcoin::Network;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};
use bdk::miniscript::ToPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use std::str::FromStr;

const BACKEND_AUTH_DERIVATION_PATH: &str = "m/76738065h/0h/0";
const ACCOUNT_DERIVATION_PATH: &str = "m/84h/1h/0h";

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

fn generate_random_bytes() -> Result<[u8; 32], MnemonicGenerationError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|e| MnemonicGenerationError::EntropyGeneration {
            message: e.to_string(),
        })?;
    Ok(bytes)
}

pub struct KeyPair {
    pub secret_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

pub struct LipaKeys {
    pub auth_keypair: KeyPair,
    pub master_xpriv: String,
    pub account_xpub: String,
}

pub fn derive_keys_for_caching(
    network: Network,
    mnemonic_string: Vec<String>,
) -> Result<LipaKeys, KeyDerivationError> {
    let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).map_err(|e| {
        KeyDerivationError::MnemonicParsing {
            message: e.to_string(),
        }
    })?;

    let auth_keypair = derive_auth_keypair(network, mnemonic.clone())?;

    let master_xpriv = get_master_xpriv(network, mnemonic.clone())?.to_string();

    let account_xpub = derive_account_xpub(network, mnemonic)?;

    Ok(LipaKeys {
        auth_keypair,
        master_xpriv,
        account_xpub,
    })
}

fn derive_auth_keypair(
    network: Network,
    mnemonic: Mnemonic,
) -> Result<KeyPair, KeyDerivationError> {
    let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

    let master_xpriv = get_master_xpriv(network, mnemonic)?;

    let lipa_purpose_path =
        DerivationPath::from_str(BACKEND_AUTH_DERIVATION_PATH).map_err(|e| {
            KeyDerivationError::DerivationPathParse {
                message: e.to_string(),
            }
        })?;
    let auth_xpriv = master_xpriv
        .derive_priv(&secp256k1, &lipa_purpose_path)
        .map_err(|e| KeyDerivationError::Derivation {
            message: e.to_string(),
        })?;

    let auth_priv_key = auth_xpriv.private_key.secret_bytes().to_vec();

    let auth_pub_key = PublicKey::from_secret_key(&secp256k1, &auth_xpriv.private_key)
        .to_public_key()
        .to_bytes();

    Ok(KeyPair {
        secret_key: auth_priv_key,
        public_key: auth_pub_key,
    })
}

fn get_master_xpriv(
    network: Network,
    mnemonic: Mnemonic,
) -> Result<ExtendedPrivKey, KeyDerivationError> {
    let master_extended_key: ExtendedKey =
        mnemonic
            .into_extended_key()
            .map_err(|e| KeyDerivationError::ExtendedKeyFromMnemonic {
                message: e.to_string(),
            })?;
    let master_xpriv = match master_extended_key.into_xprv(network) {
        None => return Err(KeyDerivationError::XPrivFromExtendedKey),
        Some(xpriv) => xpriv,
    };
    Ok(master_xpriv)
}

fn derive_account_xpub(network: Network, mnemonic: Mnemonic) -> Result<String, KeyDerivationError> {
    let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

    let master_xpriv = get_master_xpriv(network, mnemonic)?;

    let wallet_account_path = DerivationPath::from_str(ACCOUNT_DERIVATION_PATH).map_err(|e| {
        KeyDerivationError::DerivationPathParse {
            message: e.to_string(),
        }
    })?;

    let account_xpriv = master_xpriv
        .derive_priv(&secp256k1, &wallet_account_path)
        .map_err(|e| KeyDerivationError::Derivation {
            message: e.to_string(),
        })?;
    let account_xkey: ExtendedKey = account_xpriv.into_extended_key().map_err(|e| {
        KeyDerivationError::ExtendedKeyFromXPriv {
            message: e.to_string(),
        }
    })?;

    let account_xpub = account_xkey.into_xpub(network, &secp256k1);

    Ok(account_xpub.to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use bdk::bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use bdk::bitcoin::util::bip32::ExtendedPubKey;
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

    #[test]
    fn test_keys_code_decode() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let keys = derive_keys_for_caching(Network::Testnet, mnemonic_string).unwrap();

        let auth_priv_key = SecretKey::from_slice(keys.auth_keypair.secret_key.as_slice()).unwrap();
        assert_eq!(
            keys.auth_keypair.secret_key,
            auth_priv_key.secret_bytes().to_vec()
        );

        let auth_pub_key = PublicKey::from_slice(keys.auth_keypair.public_key.as_slice()).unwrap();
        assert_eq!(
            keys.auth_keypair.public_key,
            auth_pub_key.to_public_key().to_bytes()
        );

        let master_xpriv = ExtendedPrivKey::from_str(keys.master_xpriv.as_str()).unwrap();
        assert_eq!(keys.master_xpriv, master_xpriv.to_string());

        let account_xpub = ExtendedPubKey::from_str(keys.account_xpub.as_str()).unwrap();
        assert_eq!(keys.account_xpub, account_xpub.to_string());
    }

    #[test]
    fn test_auth_keys_match() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();

        let keypair = derive_auth_keypair(Network::Testnet, mnemonic).unwrap();

        let public_key_from_secret_key = PublicKey::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(keypair.secret_key.as_slice()).unwrap(),
        );

        assert_eq!(
            keypair.public_key,
            public_key_from_secret_key.to_public_key().to_bytes()
        );
    }

    #[test]
    fn test_master_and_account_derivation_match() {
        let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

        let mnemonic_string = generate_mnemonic().unwrap();

        let keys = derive_keys_for_caching(Network::Testnet, mnemonic_string).unwrap();

        let master_xpriv = ExtendedPrivKey::from_str(keys.master_xpriv.as_str()).unwrap();
        let account_xpub = ExtendedPubKey::from_str(keys.account_xpub.as_str()).unwrap();

        // `account_xpub` should be the xpub of `master_xpriv` at path "m/84h/1h/0h"
        // Deriving from the master the public key at "m/84h/1h/0h/0/0" must be equivalent to
        // deriving from the account the public key at "m/0/0"

        let path_from_master =
            DerivationPath::from_str(format!("{}{}", ACCOUNT_DERIVATION_PATH, "/0/0").as_str())
                .unwrap();
        let path_from_account = DerivationPath::from_str("m/0/0").unwrap();

        let target_xkey_from_master: ExtendedKey = master_xpriv
            .derive_priv(&secp256k1, &path_from_master)
            .unwrap()
            .into_extended_key()
            .unwrap();
        let target_pubkey_from_master = target_xkey_from_master
            .into_xpub(Network::Testnet, &secp256k1)
            .public_key;

        let target_pubkey_from_account = account_xpub
            .derive_pub(&secp256k1, &path_from_account)
            .unwrap()
            .public_key;

        assert_eq!(target_pubkey_from_account, target_pubkey_from_master);
    }
}
