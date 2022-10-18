use bdk::bitcoin::Network;
use uniffi_lipabusinesslib::{Config, Wallet};

const WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

#[test]
fn test_get_balance_testnet_electrum() {
    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database".to_string(),
        network: Network::Testnet,
        watch_descriptor: WATCH_DESCRIPTOR.to_string(),
    })
    .unwrap();

    let balance = wallet.sync_balance().unwrap();

    assert_eq!(balance.confirmed, 88009);
}
