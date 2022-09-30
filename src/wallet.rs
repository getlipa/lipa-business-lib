use crate::errors::WalletError;
use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::electrum_client::Client;
use bdk::{Balance, SyncOptions};
use sled::Tree;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct Config {
    pub electrum_url: String,
    pub network: Network,
    pub watch_descriptor: String,
    pub db_path: String,
}

pub struct Wallet {
    blockchain: ElectrumBlockchain,
    wallet: Arc<Mutex<bdk::Wallet<Tree>>>,
}

impl Wallet {
    pub fn new(config: Config) -> Result<Self, WalletError> {
        let client =
            Client::new(&config.electrum_url).map_err(|e| WalletError::ChainBackendClient {
                message: e.to_string(),
            })?;
        let blockchain = ElectrumBlockchain::from(client);

        let path = Path::new(&config.db_path);
        let database = sled::open(path).unwrap();
        let db_tree = database.open_tree("wallet").unwrap();

        let wallet = bdk::Wallet::new(&config.watch_descriptor, None, config.network, db_tree)
            .map_err(|e| WalletError::BdkWallet {
                message: e.to_string(),
            })?;
        let wallet = Arc::new(Mutex::new(wallet));

        Ok(Self { blockchain, wallet })
    }

    pub fn get_balance(&self) -> Result<Balance, WalletError> {
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

// The following test is commented out because it relies on an external service but
// it should work if uncommented.
// TODO: change test blockchain backend to a local instance
#[cfg(test)]
mod test {

    use crate::{Config, Wallet};
    use bdk::bitcoin::Network;

    const WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

    #[ignore]
    #[test]
    fn test_get_balance() {
        let wallet = Wallet::new(Config {
            electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
            network: Network::Testnet,
            watch_descriptor: WATCH_DESCRIPTOR.to_string(),
            db_path: ".bdk-database".to_string(),
        })
        .unwrap();

        let balance = wallet.get_balance().unwrap();

        assert_eq!(balance.confirmed, 88009);
    }
}
