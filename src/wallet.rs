use crate::errors::WalletError;
use crate::{AddrError, GetStatusError};
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

pub struct DrainTx {
    pub id: String,
    pub serialized_tx: Vec<u8>,
    pub fee_sat: u64,
    pub output_sat: u64,
}

pub enum TxStatus {
    NotInMempool,
    InMempool,
    Confirmed { number_of_blocks: u32 },
}

impl Wallet {
    pub fn new(config: Config) -> Result<Self, WalletError> {
        let client =
            Client::new(&config.electrum_url).map_err(|e| WalletError::ChainBackendClient {
                message: e.to_string(),
            })?;
        let blockchain = ElectrumBlockchain::from(client);

        let db_path = Path::new(&config.wallet_db_path);
        let db = sled::open(db_path).map_err(|e| WalletError::OpenDatabase {
            message: e.to_string(),
        })?;
        let db_tree =
            db.open_tree("bdk-wallet-database")
                .map_err(|e| WalletError::OpenDatabaseTree {
                    message: e.to_string(),
                })?;

        let wallet = bdk::Wallet::new(&config.watch_descriptor, None, config.network, db_tree)
            .map_err(|e| WalletError::BdkWallet {
                message: e.to_string(),
            })?;
        let wallet = Arc::new(Mutex::new(wallet));

        Ok(Self { blockchain, wallet })
    }

    pub fn sync_balance(&self) -> Result<Balance, WalletError> {
        let wallet = self.wallet.lock().unwrap();

        wallet
            .sync(&self.blockchain, SyncOptions::default())
            .map_err(|e| WalletError::ChainSync {
                message: e.to_string(),
            })?;

        let balance = wallet.get_balance().map_err(|e| WalletError::GetBalance {
            message: e.to_string(),
        })?;

        Ok(balance)
    }

    pub fn prepare_drain_tx(&self, _addr: String) -> Result<DrainTx, WalletError> {
        todo!()
    }

    pub fn sign_and_broadcast_tx(
        &self,
        _tx: DrainTx,
        _spend_descriptor: String,
    ) -> Result<(), WalletError> {
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

pub fn validate_addr(_addr: String) -> Result<(), AddrError> {
    todo!()
}

pub fn get_tx_status(_txid: String) -> Result<TxStatus, GetStatusError> {
    todo!()
}
