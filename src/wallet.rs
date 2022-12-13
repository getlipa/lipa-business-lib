use crate::errors::{LipaResult, MapToLipaError};
use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::electrum_client::Client;
use bdk::sled::Tree;
use bdk::{Balance, SyncOptions};
use std::path::Path;
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

pub enum TxStatus {
    NotInMempool,
    InMempool,
    Confirmed { number_of_blocks: u32 },
}

pub enum AddressValidationResult {
    Valid,
    Invalid,
}

impl Wallet {
    pub fn new(config: Config) -> LipaResult<Self> {
        let client = Client::new(&config.electrum_url)
            .map_to_runtime_error("Failed to create an electrum client")?;
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
        let wallet = self.wallet.lock().unwrap();

        wallet
            .sync(&self.blockchain, SyncOptions::default())
            .map_to_runtime_error("Failed to sync bdk wallet")?;

        let balance = wallet
            .get_balance()
            .map_to_runtime_error("Failed to get balance from bdk wallet")?;

        Ok(balance)
    }

    pub fn validate_addr(&self, _addr: String) -> AddressValidationResult {
        todo!()
    }

    pub fn prepare_drain_tx(&self, _addr: String) -> LipaResult<Tx> {
        todo!()
    }

    pub fn sign_and_broadcast_tx(&self, _tx: Tx, _spend_descriptor: String) -> LipaResult<()> {
        todo!()
    }

    pub fn get_tx_status(&self, _txid: String) -> LipaResult<TxStatus> {
        todo!()
    }

    // Not needed for now
    /*pub fn get_address(&self, watch_descriptor: String) -> Result<String, WalletError> {
        let wallet = bdk::Wallet::new(
            &watch_descriptor,
            None,
            self.config.network,
            MemoryDatabase::default(),
        )
        .map_err(|e| WalletError::BdkWallet {
            message: e.to_string(),
        })?;

        wallet
            .sync(&self.blockchain, SyncOptions::default())
            .map_err(|e| WalletError::ChainSync {
                message: e.to_string(),
            })?;

        let address = wallet.get_address(AddressIndex::LastUnused).unwrap().address;

        Ok(address.to_string())
    }*/
}
