use crate::errors::{permanent_failure, LipaResult, MapToLipaError};
use bdk::bitcoin::hashes::hex::ToHex;
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
use secp256k1::SECP256K1;
use std::str::FromStr;

// In the near future we want to migrate to the following keys for backend auth
//const BACKEND_AUTH_DERIVATION_PATH: &str = "m/76738065'/0'/0";
// For now, we use the master key pair
const BACKEND_AUTH_DERIVATION_PATH: &str = "m";
const ACCOUNT_DERIVATION_PATH_MAINNET: &str = "m/84'/0'/0'";
const ACCOUNT_DERIVATION_PATH_TESTNET: &str = "m/84'/1'/0'";

pub fn generate_mnemonic() -> LipaResult<Vec<String>> {
    let entropy = generate_random_bytes()?;
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_to_permanent_failure("Failed to get mnemonic from entropy")?;

    let mnemonic: Vec<String> = mnemonic.word_iter().map(|s| s.to_string()).collect();

    Ok(mnemonic)
}

fn generate_random_bytes() -> LipaResult<[u8; 32]> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_to_permanent_failure("Failed to generate random bytes using OsRng")?;
    Ok(bytes)
}

pub struct KeyPair {
    pub secret_key: String,
    pub public_key: String,
}

pub struct Descriptors {
    pub spend_descriptor: String,
    pub watch_descriptor: String,
}

pub struct WalletKeys {
    pub wallet_keypair: KeyPair,
    pub wallet_descriptors: Descriptors,
}

pub fn derive_keys(network: Network, mnemonic_string: Vec<String>) -> LipaResult<WalletKeys> {
    let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str())
        .map_to_invalid_input("Invalid mnemonic string")?;

    let master_xpriv = get_master_xpriv(network, mnemonic)?;

    let auth_keypair = derive_auth_keypair(master_xpriv)?;
    let spend_descriptor = build_spend_descriptor(network, master_xpriv)?;
    let watch_descriptor = build_watch_descriptor(network, master_xpriv)?;

    Ok(WalletKeys {
        wallet_keypair: auth_keypair,
        wallet_descriptors: Descriptors {
            spend_descriptor,
            watch_descriptor,
        },
    })
}

fn derive_auth_keypair(master_xpriv: ExtendedPrivKey) -> LipaResult<KeyPair> {
    let lipa_purpose_path = DerivationPath::from_str(BACKEND_AUTH_DERIVATION_PATH)
        .map_to_permanent_failure("Failed to build derivation path")?;

    let auth_xpriv = master_xpriv
        .derive_priv(SECP256K1, &lipa_purpose_path)
        .map_to_permanent_failure("Failed to derive keys")?;

    let auth_priv_key = auth_xpriv.private_key.secret_bytes().to_vec();

    let auth_pub_key = PublicKey::from_secret_key(SECP256K1, &auth_xpriv.private_key)
        .to_public_key()
        .to_bytes();

    Ok(KeyPair {
        secret_key: auth_priv_key.to_hex(),
        public_key: auth_pub_key.to_hex(),
    })
}

fn get_master_xpriv(network: Network, mnemonic: Mnemonic) -> LipaResult<ExtendedPrivKey> {
    let master_extended_key: ExtendedKey = mnemonic
        .into_extended_key()
        .map_to_permanent_failure("Failed to get extended key from mnemonic")?;
    let master_xpriv = match master_extended_key.into_xprv(network) {
        None => return Err(permanent_failure("Failed to get xpriv from extended key")),
        Some(xpriv) => xpriv,
    };
    Ok(master_xpriv)
}

fn build_spend_descriptor(network: Network, master_xpriv: ExtendedPrivKey) -> LipaResult<String> {
    // Directly embed the master extended key in the descriptor
    let origin_path = "m";

    // Provide a BIP84 derivation path for the descriptor. It's built from the
    // account derivation path concatenated with the "change" path ("/0")
    let key_path = format!("{}{}", get_account_derivation_path(network), "/0");

    build_descriptor(
        master_xpriv,
        origin_path,
        key_path.as_str(),
        DescriptorKind::Private,
    )
}

fn build_watch_descriptor(network: Network, master_xpriv: ExtendedPrivKey) -> LipaResult<String> {
    // Embed the account level extended key in the descriptor
    let origin_path = get_account_derivation_path(network);

    // The extended key in the descriptor is already the account-level one so we just need to set
    // the remaining part of the path
    let key_path = "m/0";

    build_descriptor(master_xpriv, origin_path, key_path, DescriptorKind::Public)
}

enum DescriptorKind {
    Public,
    Private,
}

/// Builds a descriptor
///
/// * Parameters:
/// - `master_xpriv`: Master xpriv
/// - `origin_derivation_path`: the xkey that is embedded in the descriptor will be derived
/// from the master xpriv using this path
/// - `key_derivation_path`: this is the derivation path that is applied to the embedded xkey when
/// using the built descriptor
/// - `public`: if true, the embedded xkey will be an xpub, otherwise will be an xpriv
fn build_descriptor(
    master_xpriv: ExtendedPrivKey,
    origin_derivation_path: &str,
    key_derivation_path: &str,
    kind: DescriptorKind,
) -> LipaResult<String> {
    let extended_key_derivation_path = DerivationPath::from_str(origin_derivation_path)
        .map_to_permanent_failure("Failed to build derivation path")?;
    let descriptor_derivation_path = DerivationPath::from_str(key_derivation_path)
        .map_to_permanent_failure("Failed to build derivation path")?;

    let derived_xpriv = master_xpriv
        .derive_priv(SECP256K1, &extended_key_derivation_path)
        .map_to_permanent_failure("Failed to derive keys")?;

    let origin: KeySource = (
        master_xpriv.fingerprint(SECP256K1),
        extended_key_derivation_path,
    );

    let derived_xpriv_desc_key: DescriptorKey<Segwitv0> = derived_xpriv
        .into_descriptor_key(Some(origin), descriptor_derivation_path)
        .map_to_permanent_failure("Failed to get descriptor key from xpriv")?;

    if let Secret(desc_seckey, _, _) = derived_xpriv_desc_key {
        let desc_key = match kind {
            DescriptorKind::Public => {
                let desc_pubkey = desc_seckey
                    .to_public(SECP256K1)
                    .map_to_permanent_failure("Failed to parse descriptor key")?;
                desc_pubkey.to_string()
            }
            DescriptorKind::Private => desc_seckey.to_string(),
        };
        Ok(key_to_wpkh_descriptor(&desc_key))
    } else {
        Err(permanent_failure("Failed to get descriptor from xpriv"))
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
    format!("wpkh({key})")
}

pub fn generate_keypair() -> KeyPair {
    let mut rng = rand::rngs::OsRng;

    let (secret_key, public_key) = SECP256K1.generate_keypair(&mut rng);

    KeyPair {
        secret_key: secret_key.secret_bytes().to_hex(),
        public_key: public_key.serialize().to_hex(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk::bitcoin::hashes::hex::FromHex;
    use bdk::bitcoin::secp256k1::{PublicKey, SecretKey};
    use std::str::FromStr;

    // Values used for testing were obtained from https://iancoleman.io/bip39
    const NETWORK: Network = Network::Testnet;
    const MNEMONIC_STR: &str = "between angry ketchup hill admit attitude echo wisdom still barrel coral obscure home museum trick grow magic eagle school tilt loop actress equal law";
    const SPEND_DESCRIPTOR: &str = "wpkh([aed2a027]tprv8ZgxMBicQKsPeT4bcpTNiHtBXqHRRPh4qMkWP4PahRJCGLd5A32RYUif9PJ8GMChWPB6yFFNGybZRGBFcsb9v9YifukeysfDAHDTzxRrtbi/84'/1'/0'/0/*)";
    const WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

    // The following corresponds to path "m/76738065'/0'/0"
    //const AUTH_PUB_KEY: &str = "02549b15801b155d32ca3931665361b1d2997ee531859b2d48cebbc2ccf21aac96";
    // For now we'll use the master key pair
    const AUTH_PUB_KEY: &str = "0365704b042bdf2a8bf19714902242f9275ce7b0e2438a35dbb25133c49d1c8ef2";

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

        assert_eq!(
            keys.wallet_descriptors.spend_descriptor,
            SPEND_DESCRIPTOR.to_string()
        );
        assert_eq!(
            keys.wallet_descriptors.watch_descriptor,
            WATCH_DESCRIPTOR.to_string()
        );
        assert_eq!(keys.wallet_keypair.public_key, AUTH_PUB_KEY.to_string());

        // No need to check that the auth secret_key is correct because here we check the auth
        // public key and in `test_auth_keys_match()` we check that the keys match.
    }

    #[test]
    fn test_auth_keys_encode_decode() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);

        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let auth_priv_key = SecretKey::from_slice(
            Vec::from_hex(&keys.wallet_keypair.secret_key)
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        assert_eq!(
            keys.wallet_keypair.secret_key,
            auth_priv_key.secret_bytes().to_vec().to_hex()
        );

        let auth_pub_key = PublicKey::from_slice(
            Vec::from_hex(&keys.wallet_keypair.public_key)
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        assert_eq!(
            keys.wallet_keypair.public_key,
            auth_pub_key.to_public_key().to_bytes().to_hex()
        );
    }

    fn check_keys_match(keypair: KeyPair) {
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
    fn test_auth_keys_match() {
        let mnemonic_string = mnemonic_str_to_vec(MNEMONIC_STR);
        let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();

        let master_xpriv = get_master_xpriv(NETWORK, mnemonic).unwrap();

        let keypair = derive_auth_keypair(master_xpriv).unwrap();

        check_keys_match(keypair);
    }

    #[test]
    fn test_generate_keypair() {
        let keypair = generate_keypair();

        check_keys_match(keypair);
    }
}
