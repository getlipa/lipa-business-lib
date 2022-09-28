use crate::errors::SigningError;
use bdk::bitcoin::secp256k1::{Message, SecretKey};
use bdk::bitcoin::util::misc::signed_msg_hash;

pub fn sign_message(message: String, secret_key: Vec<u8>) -> Result<String, SigningError> {
    let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

    let message = Message::from_slice(&signed_msg_hash(&message)).map_err(|e| {
        SigningError::MessageHashing {
            message: e.to_string(),
        }
    })?;
    let secret_key =
        SecretKey::from_slice(secret_key.as_slice()).map_err(|e| SigningError::SecretKeyParse {
            message: e.to_string(),
        })?;

    let sig = secp256k1.sign_ecdsa(&message, &secret_key);

    Ok(sig.to_string())
}

#[cfg(test)]
mod test {
    use crate::signing::sign_message;
    use crate::{derive_keys, generate_mnemonic};
    use bdk::bitcoin::secp256k1::ecdsa::Signature;
    use bdk::bitcoin::secp256k1::{Error, Message, PublicKey};
    use bdk::bitcoin::util::misc::signed_msg_hash;
    use bdk::bitcoin::Network;
    use std::str::FromStr;

    const NETWORK: Network = Network::Testnet;

    fn verify_sig(message: String, signature: String, public_key: Vec<u8>) -> Result<(), Error> {
        let secp256k1 = bdk::bitcoin::secp256k1::Secp256k1::new();

        let message = Message::from_slice(&*signed_msg_hash(&message)).unwrap();
        let signature = Signature::from_str(&signature).unwrap();
        let public_key = PublicKey::from_slice(public_key.as_slice()).unwrap();

        secp256k1.verify_ecdsa(&message, &signature, &public_key)
    }

    #[test]
    fn test_sign_message() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let message = String::from("Hello World!");

        let sig = sign_message(message.clone(), keys.auth_keypair.secret_key.clone()).unwrap();

        verify_sig(message, sig, keys.auth_keypair.public_key).unwrap()
    }
}
