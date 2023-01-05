use crate::errors::{invalid_input, permanent_failure, runtime_error, LipaResult, MapToLipaError};
use crate::RuntimeErrorCode;
use bdk::bitcoin::consensus::deserialize;
use bdk::bitcoin::consensus::serialize;
use bdk::bitcoin::psbt::Psbt;
use bdk::bitcoin::{Address, Network, OutPoint, Txid};
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::database::{Database, MemoryDatabase};
use bdk::electrum_client::Client;
use bdk::sled::Tree;
use bdk::wallet::AddressIndex;
use bdk::{Balance, Error, SignOptions, SyncOptions};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub struct Config {
    pub electrum_url: String,
    pub wallet_db_path: String,
    pub network: Network,
    pub watch_descriptor: String,
}

pub struct Wallet {
    blockchain: ElectrumBlockchain,
    wallet: Arc<Mutex<bdk::Wallet<Tree>>>,
}

pub struct Tx {
    pub id: String,
    pub blob: Vec<u8>,
    pub on_chain_fee_sat: u64,
    pub output_sat: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TxStatus {
    NotInMempool,
    InMempool,
    Confirmed { number_of_blocks: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddressValidationResult {
    Valid,
    Invalid,
}

impl Wallet {
    pub fn new(config: Config) -> LipaResult<Self> {
        let client = Client::new(&config.electrum_url).map_to_runtime_error(
            RuntimeErrorCode::RemoteServiceUnavailable,
            "Failed to create an electrum client",
        )?;
        let blockchain = ElectrumBlockchain::from(client);

        let db_path = Path::new(&config.wallet_db_path);
        let db = sled::open(db_path).map_to_permanent_failure("Failed to open sled database")?;
        let db_tree = db
            .open_tree("bdk-wallet-database")
            .map_to_permanent_failure("Failed to open sled database tree")?;

        let wallet = bdk::Wallet::new(&config.watch_descriptor, None, config.network, db_tree)
            .map_to_permanent_failure("Failed to create wallet")?;
        let wallet = Arc::new(Mutex::new(wallet));

        Ok(Self { blockchain, wallet })
    }

    pub fn sync_balance(&self) -> LipaResult<Balance> {
        self.sync()?;

        let wallet = self.wallet.lock().unwrap();

        let balance = wallet
            .get_balance()
            .map_to_permanent_failure("Failed to get balance from bdk wallet")?;

        Ok(balance)
    }

    pub fn validate_addr(&self, addr: String) -> AddressValidationResult {
        let address = match Address::from_str(&addr) {
            Ok(a) => a,
            Err(_) => return AddressValidationResult::Invalid,
        };

        if address.network == self.wallet.lock().unwrap().network() {
            AddressValidationResult::Valid
        } else {
            AddressValidationResult::Invalid
        }
    }

    pub fn prepare_drain_tx(&self, addr: String, confirm_in_blocks: u32) -> LipaResult<Tx> {
        let address = Address::from_str(&addr).map_to_invalid_input("Invalid bitcoin address")?;

        if !(1..=25).contains(&confirm_in_blocks) {
            return Err(invalid_input(
                "Invalid block confirmation target. Please use a target in the range [1; 25]",
            ));
        }

        let tip_height = self.sync()?;

        let fee_rate = self
            .blockchain
            .estimate_fee(confirm_in_blocks as usize)
            .map_to_runtime_error(
                RuntimeErrorCode::ElectrumServiceUnavailable,
                "Failed to estimate fee for drain tx",
            )?;

        let mut confirmed_utxo_outpoints: Vec<OutPoint> = Vec::new();

        let wallet = self.wallet.lock().unwrap();

        for utxo in wallet
            .list_unspent()
            .map_to_permanent_failure("Failed to list UTXOs")?
        {
            let txid = utxo.outpoint.txid;
            match Self::get_tx_status_internal(&wallet, txid, tip_height)? {
                TxStatus::NotInMempool => {}
                TxStatus::InMempool => {}
                TxStatus::Confirmed { .. } => {
                    confirmed_utxo_outpoints.push(utxo.outpoint);
                }
            }
        }

        let mut tx_builder = wallet.build_tx();

        tx_builder
            .add_utxos(&confirmed_utxo_outpoints)
            .map_to_permanent_failure("Failed to add utxos to tx builder")?
            .manually_selected_only()
            .drain_to(address.script_pubkey())
            .fee_rate(fee_rate)
            .enable_rbf();

        let (psbt, tx_details) = tx_builder
            .finish()
            .map_to_runtime_error(RuntimeErrorCode::NotEnoughFunds, "Failed to create PSBT")?;

        let fee = match tx_details.fee {
            None => return Err(permanent_failure("Empty fee using an Electrum backend")),
            Some(f) => f,
        };

        let tx = Tx {
            id: tx_details.txid.to_string(),
            blob: serialize(&psbt),
            on_chain_fee_sat: fee,
            output_sat: tx_details.sent - fee,
        };

        Ok(tx)
    }

    pub fn sign_and_broadcast_tx(
        &self,
        tx_blob: Vec<u8>,
        spend_descriptor: String,
    ) -> LipaResult<()> {
        let mut psbt = deserialize::<Psbt>(&tx_blob).map_to_invalid_input("Invalid tx blob")?;

        let wallet = bdk::Wallet::new(
            &spend_descriptor,
            None,
            self.wallet.lock().unwrap().network(),
            MemoryDatabase::new(),
        )
        .unwrap();

        let is_finalized = wallet
            .sign(&mut psbt, SignOptions::default())
            .map_to_permanent_failure("Failed to sign PSBT")?;
        if !is_finalized {
            return Err(permanent_failure("Wallet didn't sign all inputs"));
        }

        self.blockchain
            .broadcast(&psbt.extract_tx())
            .map_to_runtime_error(
                RuntimeErrorCode::ElectrumServiceUnavailable,
                "Failed to broadcast tx",
            )
    }

    pub fn get_tx_status(&self, txid: String) -> LipaResult<TxStatus> {
        let txid = Txid::from_str(&txid).map_to_invalid_input("Invalid tx id")?;

        let tip_height = self.sync()?;

        let wallet = self.wallet.lock().unwrap();
        Self::get_tx_status_internal(&wallet, txid, tip_height)
    }

    fn get_tx_status_internal(
        wallet: &bdk::Wallet<Tree>,
        txid: Txid,
        tip: u32,
    ) -> LipaResult<TxStatus> {
        let include_raw = false;
        let tx = wallet
            .get_tx(&txid, include_raw)
            .map_to_permanent_failure("Failed to get tx from the wallet")?;

        let status = match tx {
            None => TxStatus::NotInMempool,
            Some(tx) => match tx.confirmation_time {
                None => TxStatus::InMempool,
                Some(block_time) => {
                    debug_assert!(tip >= block_time.height);
                    let number_of_blocks = 1 + tip - block_time.height;
                    TxStatus::Confirmed { number_of_blocks }
                }
            },
        };
        Ok(status)
    }

    pub fn get_addr(&self) -> LipaResult<String> {
        self.sync()?;

        let wallet = self.wallet.lock().unwrap();

        let address = wallet
            .get_address(AddressIndex::New)
            .map_to_permanent_failure("Failed to get address from local BDK wallet")?
            .address;

        Ok(address.to_string())
    }

    fn sync(&self) -> LipaResult<u32> {
        let wallet = self.wallet.lock().unwrap();
        wallet
            .sync(&self.blockchain, SyncOptions::default())
            .map_err(|e| match e {
                Error::Electrum(_) => {
                    runtime_error(RuntimeErrorCode::ElectrumServiceUnavailable, e)
                }
                Error::Sled(e) => permanent_failure(e),
                _ => runtime_error(
                    RuntimeErrorCode::GenericError,
                    "Failed to sync the BDK wallet",
                ),
            })?;
        let sync_time = wallet
            .database()
            .get_sync_time()
            .map_to_permanent_failure("Failed to get sync time")?
            .ok_or_else(|| permanent_failure("Sync time is empty for synced wallet"))?;
        Ok(sync_time.block_time.height)
    }
}

#[cfg(test)]
pub mod test {
    use crate::{AddressValidationResult, Config, Wallet};
    use bdk::bitcoin::{Address, AddressType, Network};
    use std::fs::remove_dir_all;
    use std::str::FromStr;

    const MAINNET_WATCH_DESCRIPTOR: &str = "wpkh([ddd71d79/84'/0'/0']xpub6Cg6Y9ynKKSjZ1EwscvwerJMU1PPPcdhjr2tQ783zE31NUfAF1EMY4qiEBfKkExF3eBruUiSpGZLeCaFiJZSeh3HzAjNANx3TT8QxdN8GUd/0/*)";

    const TESTNET_WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

    const MAINNET_P2PKH_ADDR: &str = "151111ZKuNi4r9Ker4PjTMR1hf9TdwKe6W";
    const MAINNET_P2SH_ADDR: &str = "351112e6qVY9zzZ5HZGxhcYnX975AVzYxt";
    const MAINNET_P2WPKH_ADDR: &str = "bc1q42lja79elem0anu8q8s3h2n687re9jax556pcc";
    const MAINNET_P2TR_ADDR: &str =
        "bc1p0000awrdl80vv4j8tmx82sfxd58jl9mmln9wshqynk8sv9g9et3qzdpkkq";

    const TESTNET_P2PKH_ADDR: &str = "mqLMuMmLKHKfMExHVaUB7qcmhULSPAmdpH";
    const TESTNET_P2SH_ADDR: &str = "2N6cWfrWV9Kepj9vuFGQGzjoF96QtKnYY1P";
    const TESTNET_P2WPKH_ADDR: &str = "tb1q00000alt56z8fsczc67u7q0vsl0wrqt52x084l";
    const TESTNET_P2TR_ADDR: &str =
        "tb1p67fy6nmag04fvkjxtt3sjhl5zyc7t9r08jzl08jy4k703cn7pq8q39zmvg";

    const LN_INVOICE: &str = "lnbc15u1p3xnhl2pp5jptserfk3zk4qy42tlucycrfwxhydvlemu9pqr93tuzlv9cc7g3sdqsvfhkcap3xyhx7un8cqzpgxqzjcsp5f8c52y2stc300gl6s4xswtjpc37hrnnr3c9wvtgjfuvqmpm35evq9qyyssqy4lgd8tj637qcjp05rdpxxykjenthxftej7a2zzmwrmrl70fyj9hvj0rewhzj7jfyuwkwcg9g2jpwtk3wkjtwnkdks84hsnu8xps5vsq4gj5hs";

    #[test]
    fn test_address_validation_testnet() {
        let _ = remove_dir_all(".bdk-database-testnet");

        let wallet = Wallet::new(Config {
            electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
            wallet_db_path: ".bdk-database-testnet".to_string(),
            network: Network::Testnet,
            watch_descriptor: TESTNET_WATCH_DESCRIPTOR.to_string(),
        })
        .unwrap();

        // Valid addresses
        assert_eq!(
            wallet.validate_addr(TESTNET_P2PKH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2SH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2WPKH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2TR_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        // Invalid addresses due to wrong network
        assert_eq!(
            wallet.validate_addr(MAINNET_P2PKH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2SH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2WPKH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2TR_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        // Invalid due to being a BOLT11 LN invoice
        assert_eq!(
            wallet.validate_addr(LN_INVOICE.to_string()),
            AddressValidationResult::Invalid
        );
    }

    #[test]
    fn test_address_validation_mainnet() {
        let _ = remove_dir_all(".bdk-database-mainnet");

        let wallet = Wallet::new(Config {
            electrum_url: "ssl://electrum.blockstream.info:50002".to_string(),
            wallet_db_path: ".bdk-database-mainnet".to_string(),
            network: Network::Bitcoin,
            watch_descriptor: MAINNET_WATCH_DESCRIPTOR.to_string(),
        })
        .unwrap();

        // Valid addresses
        assert_eq!(
            wallet.validate_addr(MAINNET_P2PKH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2SH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2WPKH_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        assert_eq!(
            wallet.validate_addr(MAINNET_P2TR_ADDR.to_string()),
            AddressValidationResult::Valid
        );

        // Invalid addresses due to wrong network
        assert_eq!(
            wallet.validate_addr(TESTNET_P2PKH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2SH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2WPKH_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        assert_eq!(
            wallet.validate_addr(TESTNET_P2TR_ADDR.to_string()),
            AddressValidationResult::Invalid
        );

        // Invalid due to being a BOLT11 LN invoice
        assert_eq!(
            wallet.validate_addr(LN_INVOICE.to_string()),
            AddressValidationResult::Invalid
        );
    }

    #[test]
    fn test_get_addr() {
        let _ = remove_dir_all(".bdk-database-get-addr");

        let wallet = Wallet::new(Config {
            electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
            wallet_db_path: ".bdk-database-get-addr".to_string(),
            network: Network::Testnet,
            watch_descriptor: TESTNET_WATCH_DESCRIPTOR.to_string(),
        })
        .unwrap();

        let addr = wallet.get_addr().unwrap();

        assert_eq!(
            wallet.validate_addr(addr.clone()),
            AddressValidationResult::Valid
        );
        assert_eq!(Address::from_str(&addr).unwrap().network, Network::Testnet);
        assert_eq!(
            Address::from_str(&addr).unwrap().address_type().unwrap(),
            AddressType::P2wpkh
        );

        let addr_2 = wallet.get_addr().unwrap();

        assert_ne!(addr, addr_2);
    }
}
