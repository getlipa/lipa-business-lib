use crate::errors::*;
use bdk::bitcoin::blockdata::script::Script;
use bdk::bitcoin::blockdata::transaction::TxOut;
use bdk::bitcoin::consensus::{deserialize, serialize};
use bdk::bitcoin::psbt::Psbt;
use bdk::bitcoin::{Address, Network, OutPoint, Txid};
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::database::{Database, MemoryDatabase};
use bdk::electrum_client::Client;
use bdk::sled::Tree;
use bdk::wallet::AddressIndex;
use bdk::{Balance, Error, SignOptions, SyncOptions, TransactionDetails};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct Config {
    pub electrum_url: String,
    pub wallet_db_path: String,
    pub network: Network,
    pub watch_descriptor: String,
}

type BdkWallet = bdk::Wallet<Tree>;

pub struct Wallet {
    blockchain: ElectrumBlockchain,
    wallet: Arc<Mutex<BdkWallet>>,
}

pub struct Tx {
    pub id: String,
    pub blob: Vec<u8>,
    pub on_chain_fee_sat: u64,
    pub output_sat: u64,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum TxStatus {
    NotInMempool,
    InMempool,
    Confirmed {
        number_of_blocks: u32,
        confirmed_at: SystemTime,
    },
}

pub struct TxDetails {
    pub id: String,
    pub output_address: String,
    pub output_sat: u64,
    pub on_chain_fee_sat: u64,
    pub status: TxStatus,
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

        let wallet = bdk::Wallet::new(
            &config.watch_descriptor,
            Some(&get_change_descriptor_from_descriptor(
                &config.watch_descriptor,
            )?),
            config.network,
            db_tree,
        )
        .map_to_permanent_failure("Failed to create wallet")?;
        let wallet = Arc::new(Mutex::new(wallet));

        Ok(Self { blockchain, wallet })
    }

    pub fn get_balance(&self) -> LipaResult<Balance> {
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

    // To know if the local wallet has enough funds to create a drain tx, the most accurate
    // option is to actually try to prepare a drain tx.
    //
    // The main issue is that the goal is to know if a drain tx is affordable before knowing to
    // which address we want to drain to. For this reason, we try to prepare a drain tx
    // that spends to the a local wallet address. In some very unlikely edge cases, depending on
    // the destination address that is used, it could happen that the actual drain tx isn't
    // affordable.
    //
    // We are careful about dropping the prepared tx asap, as we don't want this tx to ever be signed.
    pub fn is_drain_tx_affordable(&self, confirm_in_blocks: u32) -> LipaResult<bool> {
        let local_address = {
            self.wallet
                .lock()
                .unwrap()
                .get_address(AddressIndex::Peek(0))
                .map_to_permanent_failure("Failed to get address from local wallet")?
                .address
        };

        match self.prepare_drain_tx_internal(local_address, confirm_in_blocks) {
            Ok(_) => Ok(true),
            Err(LipaError::RuntimeError {
                code: RuntimeErrorCode::NotEnoughFunds,
                ..
            }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn prepare_drain_tx(&self, addr: String, confirm_in_blocks: u32) -> LipaResult<Tx> {
        let address = Address::from_str(&addr).map_to_invalid_input("Invalid bitcoin address")?;

        if !(1..=25).contains(&confirm_in_blocks) {
            return Err(invalid_input(
                "Invalid block confirmation target. Please use a target in the range [1; 25]",
            ));
        }

        let wallet = self.wallet.lock().unwrap();
        let address_is_mine = wallet
            .is_mine(&address.script_pubkey())
            .map_to_permanent_failure("Failed to check if address belongs to the wallet")?;
        if address_is_mine {
            return Err(runtime_error(
                RuntimeErrorCode::SendToOurselves,
                "Trying to drain wallet to address belonging to the wallet",
            ));
        }
        drop(wallet); // To release the lock.

        self.prepare_drain_tx_internal(address, confirm_in_blocks)
    }

    fn prepare_drain_tx_internal(
        &self,
        address: Address,
        confirm_in_blocks: u32,
    ) -> LipaResult<Tx> {
        let fee_rate = self
            .blockchain
            .estimate_fee(confirm_in_blocks as usize)
            .map_to_runtime_error(
                RuntimeErrorCode::ElectrumServiceUnavailable,
                "Failed to estimate fee for drain tx",
            )?;

        let wallet = self.wallet.lock().unwrap();

        let confirmed_utxo_outpoints = Self::get_confirmed_utxo_outpoints(&wallet)?;

        let mut tx_builder = wallet.build_tx();

        tx_builder
            .add_utxos(&confirmed_utxo_outpoints)
            .map_to_permanent_failure("Failed to add utxos to tx builder")?
            .manually_selected_only()
            .drain_to(address.script_pubkey())
            .fee_rate(fee_rate)
            .enable_rbf()
            .allow_dust(false);

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
    ) -> LipaResult<TxDetails> {
        let mut psbt = deserialize::<Psbt>(&tx_blob).map_to_invalid_input("Invalid tx blob")?;

        let signing_wallet = bdk::Wallet::new(
            &spend_descriptor,
            Some(&get_change_descriptor_from_descriptor(&spend_descriptor)?),
            self.wallet.lock().unwrap().network(),
            MemoryDatabase::new(),
        )
        .map_to_permanent_failure("Failed to create signing-capable wallet")?;

        let is_finalized = signing_wallet
            .sign(&mut psbt, SignOptions::default())
            .map_to_permanent_failure("Failed to sign PSBT")?;
        if !is_finalized {
            return Err(permanent_failure("Wallet didn't sign all inputs"));
        }

        let tx = psbt.extract_tx();
        self.blockchain.broadcast(&tx).map_to_runtime_error(
            RuntimeErrorCode::ElectrumServiceUnavailable,
            "Failed to broadcast tx",
        )?;

        self.sync()?;
        let wallet = self.wallet.lock().unwrap();
        let include_raw = true;
        let tx = wallet
            .get_tx(&tx.txid(), include_raw)
            .map_to_permanent_failure("Failed to get tx from the wallet")?
            .ok_or_else(|| permanent_failure("Just signed tx not found"))?;
        Self::map_to_tx_details(tx, &wallet)
    }

    pub fn get_tx_status(&self, txid: String) -> LipaResult<TxStatus> {
        let txid = Txid::from_str(&txid).map_to_invalid_input("Invalid tx id")?;

        let wallet = self.wallet.lock().unwrap();
        Self::get_tx_status_internal(&wallet, txid)
    }

    pub fn get_spending_txs(&self) -> LipaResult<Vec<TxDetails>> {
        let wallet = self.wallet.lock().unwrap();

        let include_raw = true;
        let txs_details = wallet
            .list_transactions(include_raw)
            .map_to_permanent_failure("Wallet failed to list txs")?
            .into_iter()
            // If we send more than receive (plus fee) it means that there is at
            // least one foreign output.
            .filter(|tx| tx.sent > tx.received + tx.fee.unwrap_or(0))
            .map(|tx| Self::map_to_tx_details(tx, &wallet));

        let mut txs_details = try_collect(txs_details)?;
        txs_details.sort_unstable_by_key(|tx| (tx.status.clone(), tx.id.clone()));
        Ok(txs_details)
    }

    pub fn get_addr(&self) -> LipaResult<String> {
        let wallet = self.wallet.lock().unwrap();

        let address = wallet
            .get_address(AddressIndex::New)
            .map_to_permanent_failure("Failed to get address from local BDK wallet")?
            .address;

        Ok(address.to_string())
    }

    // Not stated in the UDL file -> at the moment is just used in tests
    pub fn prepare_send_tx(
        &self,
        addr: String,
        amount: u64,
        confirm_in_blocks: u32,
    ) -> LipaResult<Tx> {
        let address = Address::from_str(&addr).map_to_invalid_input("Invalid bitcoin address")?;

        if !(1..=25).contains(&confirm_in_blocks) {
            return Err(invalid_input(
                "Invalid block confirmation target. Please use a target in the range [1; 25]",
            ));
        }

        let wallet = self.wallet.lock().unwrap();
        let address_is_mine = wallet
            .is_mine(&address.script_pubkey())
            .map_to_permanent_failure("Failed to check if address belongs to the wallet")?;
        if address_is_mine {
            return Err(runtime_error(
                RuntimeErrorCode::SendToOurselves,
                "Trying to drain wallet to address belonging to the wallet",
            ));
        }
        drop(wallet); // To release the lock.

        let fee_rate = self
            .blockchain
            .estimate_fee(confirm_in_blocks as usize)
            .map_to_runtime_error(
                RuntimeErrorCode::ElectrumServiceUnavailable,
                "Failed to estimate fee for send tx",
            )?;

        let wallet = self.wallet.lock().unwrap();

        let confirmed_utxo_outpoints = Self::get_confirmed_utxo_outpoints(&wallet)?;

        let mut tx_builder = wallet.build_tx();

        tx_builder
            .add_utxos(&confirmed_utxo_outpoints)
            .map_to_permanent_failure("Failed to add utxos to tx builder")?
            .manually_selected_only()
            .add_recipient(address.script_pubkey(), amount)
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

    fn get_tx_status_internal(wallet: &bdk::Wallet<Tree>, txid: Txid) -> LipaResult<TxStatus> {
        let tip_height = Self::get_synced_tip_height(wallet)?;
        let include_raw = false;
        let tx = wallet
            .get_tx(&txid, include_raw)
            .map_to_permanent_failure("Failed to get tx from the wallet")?;
        Ok(Self::to_tx_status(tx, tip_height))
    }

    pub fn sync(&self) -> LipaResult<()> {
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
            })
    }

    fn get_synced_tip_height(wallet: &BdkWallet) -> LipaResult<u32> {
        match wallet
            .database()
            .get_sync_time()
            .map_to_permanent_failure("Failed to get sync time")?
        {
            Some(sync_time) => Ok(sync_time.block_time.height),
            None => Ok(0),
        }
    }

    fn get_confirmed_utxo_outpoints(wallet: &bdk::Wallet<Tree>) -> LipaResult<Vec<OutPoint>> {
        let mut confirmed_utxo_outpoints: Vec<OutPoint> = Vec::new();

        for utxo in wallet
            .list_unspent()
            .map_to_permanent_failure("Failed to list UTXOs")?
        {
            let txid = utxo.outpoint.txid;
            match Self::get_tx_status_internal(wallet, txid)? {
                TxStatus::NotInMempool => {}
                TxStatus::InMempool => {}
                TxStatus::Confirmed { .. } => {
                    confirmed_utxo_outpoints.push(utxo.outpoint);
                }
            }
        }

        Ok(confirmed_utxo_outpoints)
    }

    fn map_to_tx_details(tx: TransactionDetails, wallet: &BdkWallet) -> LipaResult<TxDetails> {
        let tip_height = Self::get_synced_tip_height(wallet)?;

        let raw_tx = tx
            .transaction
            .as_ref()
            .ok_or_else(|| permanent_failure("Tx does not have raw tx"))?;

        let foreign_output = Self::find_foreign_output(&raw_tx.output, wallet)?
            .ok_or_else(|| permanent_failure("None of tx outputs are foreign"))?;
        let output_address = Address::from_script(&foreign_output, wallet.network())
            .map_to_permanent_failure("Failed to build address from script")?
            .to_string();

        let on_chain_fee_sat = tx
            .fee
            .ok_or_else(|| permanent_failure("Tx does not have fee set"))?;

        if tx.sent < tx.received + on_chain_fee_sat {
            return Err(permanent_failure(
                "In the tx wallet receives more than sends",
            ));
        }
        let output_sat = tx.sent - tx.received - on_chain_fee_sat;

        Ok(TxDetails {
            id: tx.txid.to_string(),
            output_address,
            output_sat,
            on_chain_fee_sat,
            status: Self::to_tx_status(Some(tx), tip_height),
        })
    }

    fn find_foreign_output(outputs: &Vec<TxOut>, wallet: &BdkWallet) -> LipaResult<Option<Script>> {
        // Waiting for Iterator::try_find() to become stable.
        for output in outputs {
            if !wallet
                .is_mine(&output.script_pubkey)
                .map_to_permanent_failure("Failed to check if output belongs to the wallet")?
            {
                return Ok(Some(output.script_pubkey.clone()));
            }
        }
        Ok(None)
    }

    fn to_tx_status(tx: Option<TransactionDetails>, tip_height: u32) -> TxStatus {
        match tx {
            None => TxStatus::NotInMempool,
            Some(tx) => match tx.confirmation_time {
                None => TxStatus::InMempool,
                Some(block_time) => {
                    debug_assert!(tip_height >= block_time.height);
                    let number_of_blocks = 1 + tip_height - block_time.height;
                    let confirmed_at =
                        SystemTime::UNIX_EPOCH + Duration::from_secs(block_time.timestamp);
                    TxStatus::Confirmed {
                        number_of_blocks,
                        confirmed_at,
                    }
                }
            },
        }
    }
}

fn get_change_descriptor_from_descriptor(descriptor: &str) -> LipaResult<String> {
    if !descriptor.ends_with("0/*)") {
        return Err(invalid_input(
            "Invalid descriptor: Descriptor doesn't end with \"0/*)\". Could it already be a change descriptor?",
        ));
    }

    if descriptor.match_indices("0/*)").count() > 1 {
        return Err(invalid_input(
            "Invalid descriptor: Descriptor has multiple occurrences of substring \"0/*)\"",
        ));
    }

    Ok(descriptor.replacen("0/*)", "1/*)", 1))
}

// Waiting for Iterator::try_collect() to become stable.
fn try_collect<T, E, I: std::iter::IntoIterator<Item = Result<T, E>>>(
    iter: I,
) -> Result<Vec<T>, E> {
    let mut vec = Vec::new();
    for item in iter {
        vec.push(item?);
    }
    Ok(vec)
}

#[cfg(test)]
pub mod test {
    use crate::wallet::get_change_descriptor_from_descriptor;
    use crate::{AddressValidationResult, Config, Wallet};
    use bdk::bitcoin::{Address, AddressType, Network};
    use std::fs::remove_dir_all;
    use std::str::FromStr;

    const MAINNET_WATCH_DESCRIPTOR: &str = "wpkh([ddd71d79/84'/0'/0']xpub6Cg6Y9ynKKSjZ1EwscvwerJMU1PPPcdhjr2tQ783zE31NUfAF1EMY4qiEBfKkExF3eBruUiSpGZLeCaFiJZSeh3HzAjNANx3TT8QxdN8GUd/0/*)";
    const MAINNET_WATCH_DESCRIPTOR_CHANGE: &str = "wpkh([ddd71d79/84'/0'/0']xpub6Cg6Y9ynKKSjZ1EwscvwerJMU1PPPcdhjr2tQ783zE31NUfAF1EMY4qiEBfKkExF3eBruUiSpGZLeCaFiJZSeh3HzAjNANx3TT8QxdN8GUd/1/*)";

    const TESTNET_WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";
    const TESTNET_WATCH_DESCRIPTOR_CHANGE: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/1/*)";

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

    const INVALID_WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH/0/*)K924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

    #[test]
    fn test_get_change_descriptor_from_descriptor() {
        assert_eq!(
            MAINNET_WATCH_DESCRIPTOR_CHANGE,
            get_change_descriptor_from_descriptor(MAINNET_WATCH_DESCRIPTOR).unwrap()
        );

        assert_eq!(
            TESTNET_WATCH_DESCRIPTOR_CHANGE,
            get_change_descriptor_from_descriptor(TESTNET_WATCH_DESCRIPTOR).unwrap()
        );

        let result = get_change_descriptor_from_descriptor(MAINNET_WATCH_DESCRIPTOR_CHANGE);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("Invalid descriptor: Descriptor doesn't end with \"0/*)\". Could it already be a change descriptor?"));

        let result = get_change_descriptor_from_descriptor(INVALID_WATCH_DESCRIPTOR);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains(
            "Invalid descriptor: Descriptor has multiple occurrences of substring \"0/*)\""
        ));
    }
}
