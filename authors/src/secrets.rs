use bdk::bitcoin::hashes::hex::ToHex;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey};
use bdk::bitcoin::Network;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};
use bdk::miniscript::ToPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use secp256k1::SECP256K1;
use std::str::FromStr;

// In the near future we want to migrate to the following keys for backend auth
//const BACKEND_AUTH_DERIVATION_PATH: &str = "m/76738065'/0'/0";
// For now, we use the master key pair
const BACKEND_AUTH_DERIVATION_PATH: &str = "m";

pub fn generate_mnemonic() -> Vec<String> {
    let entropy = generate_random_bytes();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let mnemonic: Vec<String> = mnemonic.word_iter().map(|s| s.to_string()).collect();

    mnemonic
}

fn generate_random_bytes() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).unwrap();
    bytes
}

pub struct KeyPair {
    pub secret_key: String,
    pub public_key: String,
}

pub struct WalletKeys {
    pub wallet_keypair: KeyPair,
}

pub fn derive_keys(network: Network, mnemonic_string: Vec<String>) -> WalletKeys {
    let mnemonic = Mnemonic::from_str(mnemonic_string.join(" ").as_str()).unwrap();

    let master_xpriv = get_master_xpriv(network, mnemonic);

    let auth_keypair = derive_auth_keypair(master_xpriv);

    WalletKeys {
        wallet_keypair: auth_keypair,
    }
}

fn derive_auth_keypair(master_xpriv: ExtendedPrivKey) -> KeyPair {
    let lipa_purpose_path = DerivationPath::from_str(BACKEND_AUTH_DERIVATION_PATH).unwrap();

    let auth_xpriv = master_xpriv
        .derive_priv(SECP256K1, &lipa_purpose_path)
        .unwrap();

    let auth_priv_key = auth_xpriv.private_key.secret_bytes().to_vec();

    let auth_pub_key = PublicKey::from_secret_key(SECP256K1, &auth_xpriv.private_key)
        .to_public_key()
        .to_bytes();

    KeyPair {
        secret_key: auth_priv_key.to_hex(),
        public_key: auth_pub_key.to_hex(),
    }
}

fn get_master_xpriv(network: Network, mnemonic: Mnemonic) -> ExtendedPrivKey {
    let master_extended_key: ExtendedKey = mnemonic.into_extended_key().unwrap();
    master_extended_key.into_xprv(network).unwrap()
}

pub fn generate_keypair() -> KeyPair {
    let mut rng = rand::rngs::OsRng;

    let (secret_key, public_key) = SECP256K1.generate_keypair(&mut rng);

    KeyPair {
        secret_key: secret_key.secret_bytes().to_hex(),
        public_key: public_key.serialize().to_hex(),
    }
}
