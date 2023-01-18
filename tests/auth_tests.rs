use bdk::bitcoin::Network;
use uniffi_lipabusinesslib::{
    derive_keys, generate_keypair, generate_mnemonic, Auth, AuthLevel, KeyPair,
};

#[test]
fn test_basic_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(AuthLevel::Basic, wallet_keypair, auth_keypair);

    auth.query_token();
}

#[test]
fn test_owner_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(AuthLevel::Owner, wallet_keypair, auth_keypair);

    auth.query_token();
}

#[ignore]
#[test]
fn test_employee_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(AuthLevel::Employee, wallet_keypair, auth_keypair);

    auth.query_token();
}

fn generate_keys() -> (KeyPair, KeyPair) {
    println!("Generating keys ...");
    let mnemonic = generate_mnemonic().unwrap();
    println!("mnemonic: {:?}", mnemonic);
    let wallet_keys = derive_keys(Network::Testnet, mnemonic)
        .unwrap()
        .wallet_keypair;
    let auth_keys = generate_keypair();

    (wallet_keys, auth_keys)
}
