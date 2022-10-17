use crate::errors::{KeyDerivationError, MnemonicGenerationError};
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey};
use bdk::bitcoin::Network;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};
use bdk::miniscript::ToPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use secp256k1::hashes::hex::ToHex;
use secp256k1::SECP256K1;
use std::str::FromStr;

const BACKEND_AUTH_DERIVATION_PATH: &str = "m/76738065'/0'/0";
const ACCOUNT_DERIVATION_PATH_MAINNET: &str = "m/84'/1'/0h";
const ACCOUNT_DERIVATION_PATH_TESTNET: &str = "m/84'/1'/1h";

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
    pub secret_key: String,
    pub public_key: String,
}

pub struct LipaKeys {
    pub auth_keypair: KeyPair,
    pub master_xpriv: String,
    pub account_xpub: String,
}

pub fn derive_keys(
    network: Network,
    mnemonic_string: Vec<String>,
) -> Result<LipaKeys, KeyDerivationError> {
    let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).map_err(|e| {
        KeyDerivationError::MnemonicParsing {
            message: e.to_string(),
        }
    })?;

    let master_xpriv = get_master_xpriv(network, mnemonic)?;

    let auth_keypair = derive_auth_keypair(master_xpriv)?;

    let account_xpub = derive_account_xpub(network, master_xpriv)?;

    Ok(LipaKeys {
        auth_keypair,
        master_xpriv: master_xpriv.to_string(),
        account_xpub: account_xpub.to_string(),
    })
}

fn derive_auth_keypair(master_xpriv: ExtendedPrivKey) -> Result<KeyPair, KeyDerivationError> {
    let lipa_purpose_path =
        DerivationPath::from_str(BACKEND_AUTH_DERIVATION_PATH).map_err(|e| {
            KeyDerivationError::DerivationPathParse {
                message: e.to_string(),
            }
        })?;
    let auth_xpriv = master_xpriv
        .derive_priv(SECP256K1, &lipa_purpose_path)
        .map_err(|e| KeyDerivationError::Derivation {
            message: e.to_string(),
        })?;

    let auth_priv_key = auth_xpriv.private_key.secret_bytes().to_vec();

    let auth_pub_key = PublicKey::from_secret_key(SECP256K1, &auth_xpriv.private_key)
        .to_public_key()
        .to_bytes();

    Ok(KeyPair {
        secret_key: auth_priv_key.to_hex(),
        public_key: auth_pub_key.to_hex(),
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

fn derive_account_xpub(
    network: Network,
    master_xpriv: ExtendedPrivKey,
) -> Result<ExtendedPubKey, KeyDerivationError> {
    let account_path_str = get_account_derivation_path(network);
    let wallet_account_path = DerivationPath::from_str(account_path_str).map_err(|e| {
        KeyDerivationError::DerivationPathParse {
            message: e.to_string(),
        }
    })?;

    let account_xpriv = master_xpriv
        .derive_priv(SECP256K1, &wallet_account_path)
        .map_err(|e| KeyDerivationError::Derivation {
            message: e.to_string(),
        })?;
    let account_xkey: ExtendedKey = account_xpriv.into_extended_key().map_err(|e| {
        KeyDerivationError::ExtendedKeyFromXPriv {
            message: e.to_string(),
        }
    })?;

    let account_xpub = account_xkey.into_xpub(network, SECP256K1);

    Ok(account_xpub)
}

fn get_account_derivation_path(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => ACCOUNT_DERIVATION_PATH_MAINNET,
        Network::Testnet => ACCOUNT_DERIVATION_PATH_TESTNET,
        Network::Signet => ACCOUNT_DERIVATION_PATH_TESTNET,
        Network::Regtest => ACCOUNT_DERIVATION_PATH_TESTNET,
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use bdk::bitcoin::secp256k1::{PublicKey, SecretKey};
    use bdk::bitcoin::util::bip32::ExtendedPubKey;
    use secp256k1::hashes::hex::FromHex;
    use std::str::FromStr;

    // Values used for testing were obtained from https://iancoleman.io/bip39
    const NETWORK: Network = Network::Testnet;
    const MNEMONIC_STR: &str = "between angry ketchup hill admit attitude echo wisdom still barrel coral obscure home museum trick grow magic eagle school tilt loop actress equal law";
    const MASTER_XPRIV: &str = "tprv8ZgxMBicQKsPeT4bcpTNiHtBXqHRRPh4qMkWP4PahRJCGLd5A32RYUif9PJ8GMChWPB6yFFNGybZRGBFcsb9v9YifukeysfDAHDTzxRrtbi";
    const ACCOUNT_XPUB: &str = "tpubDCvyR4gGk5U6uqmiEPmJnYodvoGabDj9mN4mG7gTshTWC8aELcNALdtcCntH6Ro6dMv9NnevkCPsCpZ1hWifx2Mt83a1Wiy5GcYhuFd9ocq";
    const AUTH_PUB_KEY: &str = "02549b15801b155d32ca3931665361b1d2997ee531859b2d48cebbc2ccf21aac96";

    fn mnemonic_str_to_vec(mnemonic_str: &str) -> Vec<String> {
        mnemonic_str.split(' ').map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_mnemonic_generation() {
        let mnemonic_string = generate_mnemonic().unwrap();
        assert_eq!(mnemonic_string.len(), 24);
    }

    #[test]
    fn test_mnemonic_encode_decode() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();
        assert_eq!(mnemonic_string.join(" "), mnemonic.to_string());
    }

    #[test]
    fn test_derive_keys() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);

        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        assert_eq!(keys.master_xpriv, MASTER_XPRIV.to_string());
        assert_eq!(keys.account_xpub, ACCOUNT_XPUB.to_string());
        assert_eq!(keys.auth_keypair.public_key, AUTH_PUB_KEY);

        // No need to check that the auth secret_key is correct because here we check the auth
        // public key and in `test_auth_keys_match()` we check that the keys match.
    }

    #[test]
    fn test_keys_encode_decode() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);

        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let auth_priv_key = SecretKey::from_slice(
            Vec::from_hex(&keys.auth_keypair.secret_key)
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        assert_eq!(
            keys.auth_keypair.secret_key,
            auth_priv_key.secret_bytes().to_vec().to_hex()
        );

        let auth_pub_key = PublicKey::from_slice(
            Vec::from_hex(&keys.auth_keypair.public_key)
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        assert_eq!(
            keys.auth_keypair.public_key,
            auth_pub_key.to_public_key().to_bytes().to_hex()
        );

        let master_xpriv = ExtendedPrivKey::from_str(keys.master_xpriv.as_str()).unwrap();
        assert_eq!(keys.master_xpriv, master_xpriv.to_string());

        let account_xpub = ExtendedPubKey::from_str(keys.account_xpub.as_str()).unwrap();
        assert_eq!(keys.account_xpub, account_xpub.to_string());
    }

    #[test]
    fn test_auth_keys_match() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();

        let master_xpriv = get_master_xpriv(NETWORK, mnemonic).unwrap();

        let keypair = derive_auth_keypair(master_xpriv).unwrap();

        let public_key_from_secret_key = PublicKey::from_secret_key(
            SECP256K1,
            &SecretKey::from_slice(Vec::from_hex(&keypair.secret_key).unwrap().as_slice()).unwrap(),
        );

        assert_eq!(
            keypair.public_key,
            public_key_from_secret_key
                .to_public_key()
                .to_bytes()
                .to_hex()
        );
    }

    #[test]
    fn test_master_and_account_derivation_match() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);

        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let master_xpriv = ExtendedPrivKey::from_str(keys.master_xpriv.as_str()).unwrap();
        let account_xpub = ExtendedPubKey::from_str(keys.account_xpub.as_str()).unwrap();

        // `account_xpub` should be the xpub of `master_xpriv` at path "m/84'/1'/0'"
        // Deriving from the master the public key at "m/84'/1'/0'/0/0" must be equivalent to
        // deriving from the account the public key at "m/0/0"

        let account_path_str = get_account_derivation_path(NETWORK);
        let path_from_master =
            DerivationPath::from_str(format!("{}{}", account_path_str, "/0/0").as_str()).unwrap();
        let path_from_account = DerivationPath::from_str("m/0/0").unwrap();

        let target_xkey_from_master: ExtendedKey = master_xpriv
            .derive_priv(SECP256K1, &path_from_master)
            .unwrap()
            .into_extended_key()
            .unwrap();
        let target_pubkey_from_master = target_xkey_from_master
            .into_xpub(NETWORK, SECP256K1)
            .public_key;

        let target_pubkey_from_account = account_xpub
            .derive_pub(SECP256K1, &path_from_account)
            .unwrap()
            .public_key;

        assert_eq!(target_pubkey_from_account, target_pubkey_from_master);
    }
}
