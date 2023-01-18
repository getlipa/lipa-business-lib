use authors::secrets::{derive_keys, generate_keypair, generate_mnemonic, KeyPair};
use authors::{Auth, AuthLevel};
use bdk::bitcoin::Network;
use std::env;

#[test]
fn test_basic_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(
        get_backend_url(),
        AuthLevel::Basic,
        wallet_keypair,
        auth_keypair,
    );

    auth.query_token();
}

#[test]
fn test_owner_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(
        get_backend_url(),
        AuthLevel::Owner,
        wallet_keypair,
        auth_keypair,
    );

    auth.query_token();
}

#[test]
#[ignore]
fn test_employee_auth() {
    let (wallet_keypair, auth_keypair) = generate_keys();

    let auth = Auth::new(
        get_backend_url(),
        AuthLevel::Employee,
        wallet_keypair,
        auth_keypair,
    );

    auth.query_token();
}

fn generate_keys() -> (KeyPair, KeyPair) {
    println!("Generating keys ...");
    let mnemonic = generate_mnemonic();
    println!("mnemonic: {:?}", mnemonic);
    let wallet_keys = derive_keys(Network::Testnet, mnemonic).wallet_keypair;
    let auth_keys = generate_keypair();

    (wallet_keys, auth_keys)
}

fn get_backend_url() -> String {
    env::var("GRAPHQL_API_URL").expect("GRAPHQL_API_URL environment variable is not set")
}
