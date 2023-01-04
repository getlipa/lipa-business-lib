mod setup;

use bdk::bitcoin::consensus::deserialize;
use bdk::bitcoin::psbt::Psbt;
use bdk::bitcoin::{Address, Network, Txid};
use std::fs::remove_dir_all;
use std::str::FromStr;
use uniffi_lipabusinesslib::RuntimeErrorCode::NotEnoughFunds;
use uniffi_lipabusinesslib::{Config, LipaError, Wallet};

const WATCH_DESCRIPTOR_WITH_FUNDS: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

const WATCH_DESCRIPTOR_WITHOUT_FUNDS: &str = "wpkh([e6224ca3/84'/1'/0']tpubDCvC1cs5x9Jf3k3WKSPtg3dinzNZ5xfnfKRjWzPV8ckXewY2eKAAb4g3HTb3HLBVBmUy688fYGU3LJjDtqrSiuDzM1wi8JBQoTYLL8KSYSc/0/*)";

#[test]
fn test_get_balance_testnet_electrum() {
    let _ = remove_dir_all(".bdk-database-get-balance");

    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database-get-balance".to_string(),
        network: Network::Testnet,
        watch_descriptor: WATCH_DESCRIPTOR_WITH_FUNDS.to_string(),
    })
    .unwrap();

    let balance = wallet.sync_balance().unwrap();

    assert_eq!(balance.confirmed, 88009);
}

const TESTNET_ADDR: &str = "tb1q3ctet25lk00cmvrtkmu9dmah2kj077m4n4aqtm";

#[test]
fn test_prepare_drain_tx() {
    let _ = remove_dir_all(".bdk-database-prepare-drain-tx");

    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database-prepare-drain-tx".to_string(),
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
    let _ = remove_dir_all(".bdk-database-drain-empty-wallet");

    let wallet = Wallet::new(Config {
        electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
        wallet_db_path: ".bdk-database-drain-empty-wallet".to_string(),
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

// Caution: Run these tests sequentially, otherwise they will corrupt each other,
//      because they are manipulating their environment:
//      cargo test --features nigiri -- --test-threads 1
#[cfg(feature = "nigiri")]
mod nigiri_tests {
    use crate::setup::nigiri;
    use bdk::bitcoin::consensus::deserialize;
    use bdk::bitcoin::psbt::Psbt;
    use bdk::bitcoin::{Address, Network};
    use bdk::Balance;
    use std::fs::remove_dir_all;
    use std::str::FromStr;
    use std::thread::sleep;
    use std::time::Duration;
    use uniffi_lipabusinesslib::{Config, TxStatus, Wallet};

    const REGTEST_WATCH_DESCRIPTOR: &str = "wpkh([aeaaaa34/84'/1'/0']tpubDD9QqCT2Y9P3BV7o8a8ajDqHmwWq5XAHKsunr9vjGVYKiRdFQqqC9wuq7jgKdUi8YesiTHiAkNurq7mx7dLDGRCxY4v8fbSa8ZS53MxLrP2/0/*)";
    const REGTEST_SPEND_DESCRIPTOR: &str = "wpkh([aeaaaa34]tprv8ZgxMBicQKsPd8WGzHdgwybWcHrnFkedrEpLTrVR2hfeVPcNUV7K3TT8oSVuNAuotQAevK5S34gWtaMKGoreD2Sq7Mp5HnXqMfxwfiDnVBF/84'/1'/0'/0/*)";

    const REGTEST_TARGET_ADDR: &str = "bcrt1q2f0wx5xss0sph7ev6cmxtpt423vlk9q0th8waj";

    #[test]
    fn test_drain_flow() {
        let _ = remove_dir_all(".bdk-database-drain-funds");

        nigiri::start();

        let wallet = Wallet::new(Config {
            electrum_url: "localhost:50000".to_string(),
            wallet_db_path: ".bdk-database-drain-funds".to_string(),
            network: Network::Regtest,
            watch_descriptor: REGTEST_WATCH_DESCRIPTOR.to_string(),
        })
        .unwrap();

        let our_addr = wallet.get_addr().unwrap();

        let tx_id_confirmed1 = nigiri::fund_address(0.1, &our_addr).unwrap();
        let tx_id_confirmed2 = nigiri::fund_address(0.1, &our_addr).unwrap();
        let tx_id_unconfirmed1 = nigiri::fund_address_without_conf(0.05, &our_addr).unwrap();
        let tx_id_unconfirmed2 = nigiri::fund_address_without_conf(0.05, &our_addr).unwrap();
        nigiri::wait_for_electrum_to_see_tx(&tx_id_confirmed1);
        nigiri::wait_for_electrum_to_see_tx(&tx_id_confirmed2);
        nigiri::wait_for_electrum_to_see_tx(&tx_id_unconfirmed1);
        nigiri::wait_for_electrum_to_see_tx(&tx_id_unconfirmed2);

        assert_eq!(
            wallet.sync_balance().unwrap(),
            Balance {
                immature: 0,
                trusted_pending: 0,
                untrusted_pending: 10_000_000,
                confirmed: 20_000_000,
            }
        );

        let drain_tx = wallet
            .prepare_drain_tx(REGTEST_TARGET_ADDR.to_string(), 1)
            .unwrap();

        assert_eq!(drain_tx.output_sat + drain_tx.on_chain_fee_sat, 20_000_000);

        let psbt = deserialize::<Psbt>(&drain_tx.blob).unwrap();

        assert_eq!(psbt.unsigned_tx.output.len(), 1);
        assert_eq!(
            psbt.unsigned_tx.output.get(0).unwrap().value,
            drain_tx.output_sat
        );
        assert_eq!(
            psbt.unsigned_tx.output.get(0).unwrap().script_pubkey,
            Address::from_str(REGTEST_TARGET_ADDR)
                .unwrap()
                .script_pubkey()
        );

        assert_eq!(
            wallet.get_tx_status(drain_tx.id.clone()).unwrap(),
            TxStatus::NotInMempool
        );

        wallet
            .sign_and_broadcast_tx(drain_tx.blob, REGTEST_SPEND_DESCRIPTOR.to_string())
            .unwrap();

        assert_eq!(
            wallet.sync_balance().unwrap(),
            Balance {
                immature: 0,
                trusted_pending: 0,
                untrusted_pending: 10_000_000,
                confirmed: 0
            }
        );

        assert_eq!(
            wallet.get_tx_status(drain_tx.id.clone()).unwrap(),
            TxStatus::InMempool
        );

        nigiri::mine_blocks(1).unwrap();
        sleep(Duration::from_secs(5));

        assert_eq!(
            wallet.sync_balance().unwrap(),
            Balance {
                immature: 0,
                trusted_pending: 0,
                untrusted_pending: 0,
                confirmed: 10_000_000
            }
        );

        assert_eq!(
            wallet.get_tx_status(drain_tx.id.clone()).unwrap(),
            TxStatus::Confirmed {
                number_of_blocks: 1
            }
        );

        nigiri::mine_blocks(1).unwrap();
        sleep(Duration::from_secs(5));

        assert_eq!(
            wallet.get_tx_status(drain_tx.id.clone()).unwrap(),
            TxStatus::Confirmed {
                number_of_blocks: 2
            }
        );

        nigiri::mine_blocks(10).unwrap();
        sleep(Duration::from_secs(5));

        assert_eq!(
            wallet.get_tx_status(drain_tx.id).unwrap(),
            TxStatus::Confirmed {
                number_of_blocks: 12
            }
        );
    }
}
