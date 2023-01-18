//use crate::errors::{LipaResult, MapToLipaError};
use bdk::bitcoin::hashes::hex::FromHex;
use bdk::bitcoin::hashes::sha256;
use bdk::bitcoin::secp256k1::Message;
use bdk::bitcoin::secp256k1::SecretKey;
use secp256k1::SECP256K1;

/*pub fn sign(message: String, private_key: String) -> LipaResult<String> {
    let message = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
    let secret_key_bytes =
        Vec::from_hex(&private_key).map_to_invalid_input("Invalid private key string")?;
    let secret_key = SecretKey::from_slice(secret_key_bytes.as_slice())
        .map_to_invalid_input("Invalid private key string")?;

    let sig = SECP256K1.sign_ecdsa(&message, &secret_key);

    Ok(sig.serialize_der().to_string())
}*/

pub fn sign(message: String, private_key: String) -> String {
    let message = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
    let secret_key_bytes = Vec::from_hex(&private_key).unwrap();
    let secret_key = SecretKey::from_slice(secret_key_bytes.as_slice()).unwrap();

    let sig = SECP256K1.sign_ecdsa(&message, &secret_key);

    sig.serialize_der().to_string()
}
/*
#[cfg(test)]
mod test {
    use crate::signing::sign;
    use crate::{derive_keys, generate_mnemonic};
    use bdk::bitcoin::hashes::hex::FromHex;
    use bdk::bitcoin::hashes::sha256;
    use bdk::bitcoin::secp256k1::ecdsa::Signature;
    use bdk::bitcoin::secp256k1::{Error, Message, PublicKey};
    use bdk::bitcoin::Network;
    use secp256k1::SECP256K1;
    use std::str::FromStr;

    const MESSAGE_STR: &str = "Hello world!";

    const NETWORK: Network = Network::Testnet;

    // Values obtained/confirmed from/on https://kjur.github.io/jsrsasign/sample/sample-ecdsa.html
    const EC_PRIVATE_KEY_HEX: &str =
        "969063eb7417a919e904a023eaef42bcd6a0d3d67598234b8fa2914ce3bda835";
    const EC_PUBLIC_KEY_HEX: &str =
        "04e2ad1cab160ee32e9840801ef200629cb4cca2e9945dd549d7955218a0876099f1bb5cf86cd694d0cdc74f91eca1acd9d25cf0e6d295b7a68e368ab79cd30e06";
    const SIG_GOLDEN: &str = "30440220059114b338f0c3f4449d76d75db28593c2e0419378f254fe5537f51180beaf7202202845666cd96056d90e8664c1d4af712a05bfa93a88907b762bd00a4366944c41";

    // Values obtained from https://gitlab.com/getlipa/api/session/-/blob/develop/docs/auth-flow.adoc
    const AUTH_PRIVATE_KEY_HEX: &str =
        "eaec97b1abc09d70a582c9459972dfb28b1be1cdfc2dace4f853a47c54af1891";
    const AUTH_PUB_KEY_HEX: &str = "0498cc3defcb5facb3b6a9042f61c9a85593f803fee89338eeceabf351d6db380598e44320c65e1765ebcdf2a5a7eb5fdeffd262732e48ef04828574e486c62f03";
    const CHALLENGE_WITH_PREFIX: &str = "x18Bitcoin Signed Message:\neyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NksifQ.eyJyZCI6IlZLZDExeEI1MXlkb2hiQTNibThMTmZNaFVOWXNEWXdFZi9ITkV6MDloZkcrVkV5MlV4THMvMlUwb3A5NkdGMkxmODZ2RkhpNWd2dGhsbG0zTTVXQjByZ1FjRmluOXljSTFoemFMRHNOdnhUaFoyY0FJL1B4cjNWS2J2YmpOR2dKVVJORytmSHVSWTk0K2RjWUxtemVRYzlRcy9POWVxQXdOSnoxQnFtbVFabDRaY1dwTE10MDc2eExtSXFiYTV3VXdpcmFnMzMyYlNMRW00ODAvdUp6OVpBeXBDS0dyY0NtUGlRQno4Y2lKeURpaUdKdGYzajZrTjN1N1cwS0l6bjZ0UFNzZndmaUNheG9FRFptZXpJYkJaSFRaUGtOUjFrZ2pvTG9nUzZGOEdDRlY5QXBHNWZERVA0bCtpb2piT05QcHEzakhjaHY4eTdCdFhXc3RBbTdGUT09IiwiZXhwIjoxNjQ5MTQzODM0fQ.MEQCIAURfxuhMcc0VtsfNCXLuTVC_l8HKocJuSNNn2n6t8n8AiB5yWezmYIsgsMa2aGSY-TjGOgQy7JP_8sTnQRsrp0IaA";
    const SIGNED_CHALLENGE_GOLDEN: &str = "3045022100a08d08cb2a6afb5592e7d911ecbc0373bc601e4b5dd3515e3d45ed5d3c708b760220765e6477a88f25594d068a0782c47755830b84059c902879ba349dab4dc8b699";

    fn verify_sig(message: String, signature: String, public_key: String) -> Result<(), Error> {
        let message = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());
        let signature = Signature::from_str(&signature).unwrap();
        let public_key =
            PublicKey::from_slice(Vec::from_hex(&public_key).unwrap().as_slice()).unwrap();

        SECP256K1.verify_ecdsa(&message, &signature, &public_key)
    }

    #[test]
    fn test_sign_message() {
        let mnemonic_string = generate_mnemonic().unwrap();
        let keys = derive_keys(NETWORK, mnemonic_string).unwrap();

        let message = String::from(MESSAGE_STR);

        let sig = sign(message.clone(), keys.wallet_keypair.secret_key.clone()).unwrap();

        verify_sig(message, sig, keys.wallet_keypair.public_key).unwrap()
    }

    #[test]
    fn test_sign_message_precomputed_value() {
        let private_key = EC_PRIVATE_KEY_HEX.to_string();
        let public_key = EC_PUBLIC_KEY_HEX.to_string();

        let sig = sign(MESSAGE_STR.to_string(), private_key).unwrap();

        verify_sig(MESSAGE_STR.to_string(), sig.clone(), public_key).unwrap();
        assert_eq!(sig, SIG_GOLDEN.to_string());
    }

    #[test]
    fn test_sign_challenge_precomputed_value() {
        let private_key = AUTH_PRIVATE_KEY_HEX.to_string();
        let public_key = AUTH_PUB_KEY_HEX.to_string();

        let sig = sign(CHALLENGE_WITH_PREFIX.to_string(), private_key).unwrap();

        verify_sig(CHALLENGE_WITH_PREFIX.to_string(), sig.clone(), public_key).unwrap();
        assert_eq!(sig, SIGNED_CHALLENGE_GOLDEN.to_string());
    }
}
*/
