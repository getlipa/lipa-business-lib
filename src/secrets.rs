use crate::errors::{KeyDerivationError, MnemonicGenerationError};
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, KeySource};
use bdk::bitcoin::Network;
use bdk::descriptor::Segwitv0;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::DescriptorKey::Secret;
use bdk::keys::{DerivableKey, DescriptorKey, ExtendedKey};
use bdk::miniscript::ToPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use std::str::FromStr;

const BACKEND_AUTH_DERIVATION_PATH: &str = "m/76738065'/0'/0";
const ACCOUNT_DERIVATION_PATH_MAINNET: &str = "m/84'/0'/0'";
const ACCOUNT_DERIVATION_PATH_TESTNET: &str = "m/84'/1'/0'";

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

pub struct Descriptors {
    pub spend_descriptor: String,
    pub watch_descriptor: String,
}

pub struct LipaKeys {
    pub auth_keypair: KeyPair,
    pub wallet_descriptors: Descriptors,
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
    let spend_descriptor = build_descriptor(
        master_xpriv,
        "m",
        format!("{}{}", get_account_derivation_path(network), "/0").as_str(),
        false,
    )?;
    let watch_descriptor = build_descriptor(
        master_xpriv,
        get_account_derivation_path(network),
        "m/0",
        true,
    )?;

    Ok(LipaKeys {
        auth_keypair,
        wallet_descriptors: Descriptors {
            spend_descriptor,
            watch_descriptor,
        },
    })
}

fn derive_auth_keypair(master_xpriv: ExtendedPrivKey) -> Result<KeyPair, KeyDerivationError> {
    let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

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

fn build_descriptor(
    master_xpriv: ExtendedPrivKey,
    extended_key_derivation_path: &str,
    descriptor_derivation_path: &str,
    public: bool,
) -> Result<String, KeyDerivationError> {
    let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

    let extended_key_derivation_path = DerivationPath::from_str(extended_key_derivation_path)
        .map_err(|e| KeyDerivationError::DerivationPathParse {
            message: e.to_string(),
        })?;
    let descriptor_derivation_path =
        DerivationPath::from_str(descriptor_derivation_path).map_err(|e| {
            KeyDerivationError::DerivationPathParse {
                message: e.to_string(),
            }
        })?;

    let derived_xpriv = master_xpriv
        .derive_priv(&secp256k1, &extended_key_derivation_path)
        .map_err(|e| KeyDerivationError::Derivation {
            message: e.to_string(),
        })?;

    let origin: KeySource = (
        master_xpriv.fingerprint(&secp256k1),
        extended_key_derivation_path,
    );

    let derived_xpriv_desc_key: DescriptorKey<Segwitv0> = derived_xpriv
        .into_descriptor_key(Some(origin), descriptor_derivation_path)
        .map_err(|e| KeyDerivationError::DescriptorKeyFromXPriv {
            message: e.to_string(),
        })?;

    if let Secret(desc_seckey, _, _) = derived_xpriv_desc_key {
        let desc_key = match public {
            true => {
                let desc_pubkey = desc_seckey.as_public(&secp256k1).map_err(|e| {
                    KeyDerivationError::DescPubKeyFromDescSecretKey {
                        message: e.to_string(),
                    }
                })?;
                desc_pubkey.to_string()
            }
            false => desc_seckey.to_string(),
        };
        Ok(key_to_wpkh_descriptor(&desc_key))
    } else {
        Err(KeyDerivationError::DescSecretKeyFromDescKey)
    }
}

fn get_account_derivation_path(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => ACCOUNT_DERIVATION_PATH_MAINNET,
        Network::Testnet => ACCOUNT_DERIVATION_PATH_TESTNET,
        Network::Signet => ACCOUNT_DERIVATION_PATH_TESTNET,
        Network::Regtest => ACCOUNT_DERIVATION_PATH_TESTNET,
    }
}

fn key_to_wpkh_descriptor(key: &str) -> String {
    let mut desc = "wpkh(".to_string();
    desc.push_str(key);
    desc.push(')');
    desc
}

#[cfg(test)]
mod test {
    use super::*;
    use bdk::bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use std::str::FromStr;

    // Values used for testing were obtained from https://iancoleman.io/bip39
    const NETWORK: Network = Network::Testnet;
    const MNEMONIC_STR: &str = "between angry ketchup hill admit attitude echo wisdom still barrel coral obscure home museum trick grow magic eagle school tilt loop actress equal law";
    const SPEND_DESCRIPTOR: &str = "wpkh([aed2a027]tprv8ZgxMBicQKsPeT4bcpTNiHtBXqHRRPh4qMkWP4PahRJCGLd5A32RYUif9PJ8GMChWPB6yFFNGybZRGBFcsb9v9YifukeysfDAHDTzxRrtbi/84'/1'/0'/0/*)";
    const WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";
    const AUTH_PUB_KEY: &str = "02549b15801b155d32ca3931665361b1d2997ee531859b2d48cebbc2ccf21aac96";

    fn mnemonic_str_to_vec(mnemonic_str: &str) -> Vec<String> {
        mnemonic_str.split(' ').map(|s| s.to_string()).collect()
    }

    fn to_vec(hex: &str) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(hex.len() / 2);

        let mut b = 0;
        for (idx, c) in hex.as_bytes().iter().enumerate() {
            b <<= 4;
            match *c {
                b'A'..=b'F' => b |= c - b'A' + 10,
                b'a'..=b'f' => b |= c - b'a' + 10,
                b'0'..=b'9' => b |= c - b'0',
                _ => return None,
            }
            if (idx & 1) == 1 {
                out.push(b);
                b = 0;
            }
        }

        Some(out)
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

        assert_eq!(
            keys.wallet_descriptors.spend_descriptor,
            SPEND_DESCRIPTOR.to_string()
        );
        assert_eq!(
            keys.wallet_descriptors.watch_descriptor,
            WATCH_DESCRIPTOR.to_string()
        );
        assert_eq!(keys.auth_keypair.public_key, to_vec(AUTH_PUB_KEY).unwrap());

        // No need to check that the auth secret_key is correct because here we check the auth
        // public key and in `test_auth_keys_match()` we check that the keys match.
    }

    #[test]
    fn test_auth_keys_encode_decode() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);

        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

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
    }

    #[test]
    fn test_auth_keys_match() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();

        let master_xpriv = get_master_xpriv(NETWORK, mnemonic).unwrap();

        let keypair = derive_auth_keypair(master_xpriv).unwrap();

        let public_key_from_secret_key = PublicKey::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_slice(keypair.secret_key.as_slice()).unwrap(),
        );

        assert_eq!(
            keypair.public_key,
            public_key_from_secret_key.to_public_key().to_bytes()
        );
    }
}
