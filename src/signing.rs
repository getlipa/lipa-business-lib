use crate::errors::SigningError;
use bdk::bitcoin::secp256k1::SecretKey;
use secp256k1::hashes::sha256;
use secp256k1::Message;

pub fn sign_message(message: String, secret_key: Vec<u8>) -> Result<String, SigningError> {
    let secp256k1 = secp256k1::Secp256k1::new();

    let message = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
    let secret_key =
        SecretKey::from_slice(secret_key.as_slice()).map_err(|e| SigningError::SecretKeyParse {
            message: e.to_string(),
        })?;

    let sig = secp256k1.sign_ecdsa(&message, &secret_key);

    Ok(sig.to_string())
}

#[cfg(test)]
mod test {
    use crate::secrets::test::to_vec;
    use crate::signing::sign_message;
    use crate::{derive_keys, generate_mnemonic};
    use bdk::bitcoin::secp256k1::ecdsa::Signature;
    use bdk::bitcoin::secp256k1::{Error, Message, PublicKey};
    use bdk::bitcoin::Network;
    use secp256k1::hashes::sha256;
    use std::str::FromStr;

    const MESSAGE_STR: &str = "Hello world!";

    const NETWORK: Network = Network::Testnet;

    // Values obtained/confirmed from/on https://kjur.github.io/jsrsasign/sample/sample-ecdsa.html
    const EC_PRIVATE_KEY_HEX: &str =
        "969063eb7417a919e904a023eaef42bcd6a0d3d67598234b8fa2914ce3bda835";
    const EC_PUBLIC_KEY_HEX: &str =
        "04e2ad1cab160ee32e9840801ef200629cb4cca2e9945dd549d7955218a0876099f1bb5cf86cd694d0cdc74f91eca1acd9d25cf0e6d295b7a68e368ab79cd30e06";
    const SIG_GOLDEN: &str = "30440220059114b338f0c3f4449d76d75db28593c2e0419378f254fe5537f51180beaf7202202845666cd96056d90e8664c1d4af712a05bfa93a88907b762bd00a4366944c41";

    fn verify_sig(message: String, signature: String, public_key: Vec<u8>) -> Result<(), Error> {
        let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

        let message = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
        let signature = Signature::from_str(&signature).unwrap();
        let public_key = PublicKey::from_slice(public_key.as_slice()).unwrap();

        secp256k1.verify_ecdsa(&message, &signature, &public_key)
    }

    #[test]
    fn test_sign_message() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let message = String::from(MESSAGE_STR);

        let sig = sign_message(message.clone(), keys.auth_keypair.secret_key.clone()).unwrap();

        verify_sig(message, sig, keys.auth_keypair.public_key).unwrap()
    }

    #[test]
    fn test_sign_message_precomputed_value() {
        let private_key = to_vec(EC_PRIVATE_KEY_HEX).unwrap();
        let public_key = to_vec(EC_PUBLIC_KEY_HEX).unwrap();

        let sig = sign_message(MESSAGE_STR.to_string(), private_key).unwrap();

        verify_sig(MESSAGE_STR.to_string(), sig.clone(), public_key).unwrap();
        assert_eq!(sig, SIG_GOLDEN.to_string());
    }
}
