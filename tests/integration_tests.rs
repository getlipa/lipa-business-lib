use bdk::bitcoin::consensus::deserialize;
use bdk::bitcoin::psbt::Psbt;
use bdk::bitcoin::{Address, Network, Txid};
use std::str::FromStr;
use uniffi_lipabusinesslib::RuntimeErrorCode::NotEnoughFunds;
use uniffi_lipabusinesslib::{Config, LipaError, Wallet};

const WATCH_DESCRIPTOR_WITH_FUNDS: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

const WATCH_DESCRIPTOR_WITHOUT_FUNDS: &str = "wpkh([e6224ca3/84'/1'/0']tpubDCvC1cs5x9Jf3k3WKSPtg3dinzNZ5xfnfKRjWzPV8ckXewY2eKAAb4g3HTb3HLBVBmUy688fYGU3LJjDtqrSiuDzM1wi8JBQoTYLL8KSYSc/0/*)";

#[test]
fn test_get_balance_testnet_electrum() {
    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database".to_string(),
        network: Network::Testnet,
        watch_descriptor: WATCH_DESCRIPTOR_WITH_FUNDS.to_string(),
    })
    .unwrap();

    let balance = wallet.sync_balance().unwrap();

    assert_eq!(balance.confirmed, 88009);
}

const TESTNET_ADDR: &str = "tb1q3ctet25lk00cmvrtkmu9dmah2kj077m4n4aqtm";

#[test]
fn test_drain_wallet() {
    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database2".to_string(),
        network: Network::Testnet,
        watch_descriptor: WATCH_DESCRIPTOR_WITH_FUNDS.to_string(),
    })
    .unwrap();

    let drain_tx = wallet
        .prepare_drain_tx(TESTNET_ADDR.to_string(), 1)
        .unwrap();

    assert_eq!(drain_tx.output_sat + drain_tx.on_chain_fee_sat, 88009);

    let psbt = deserialize::<Psbt>(&drain_tx.blob).unwrap();

    assert_eq!(
        psbt.unsigned_tx.txid(),
        Txid::from_str(&drain_tx.id).unwrap()
    );

    assert_eq!(psbt.unsigned_tx.output.len(), 1);
    assert_eq!(
        psbt.unsigned_tx.output.get(0).unwrap().value,
        drain_tx.output_sat
    );
    assert_eq!(
        psbt.unsigned_tx.output.get(0).unwrap().script_pubkey,
        Address::from_str(TESTNET_ADDR).unwrap().script_pubkey()
    );
}

#[test]
fn test_drain_empty_wallet() {
    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database3".to_string(),
        network: Network::Testnet,
        watch_descriptor: WATCH_DESCRIPTOR_WITHOUT_FUNDS.to_string(),
    })
    .unwrap();

    let drain_tx_result = wallet.prepare_drain_tx(TESTNET_ADDR.to_string(), 1);

    assert!(drain_tx_result.is_err());
    assert!(matches!(
        drain_tx_result,
        Err(LipaError::RuntimeError {
            code: NotEnoughFunds,
            ..
        })
    ));
}
