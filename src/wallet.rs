use crate::errors::WalletError;
use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::{Balance, SyncOptions};

pub struct Config {
    pub electrum_url: String,
    pub network: Network,
}

pub struct Wallet {
    config: Config,
    blockchain: ElectrumBlockchain,
}

impl Wallet {
    pub fn new(config: Config) -> Result<Self, WalletError> {
        let client =
            Client::new(&config.electrum_url).map_err(|e| WalletError::ChainBackendClient {
                message: e.to_string(),
            })?;
        let blockchain = ElectrumBlockchain::from(client);

        Ok(Self { config, blockchain })
    }

    pub fn get_balance(&self, watch_descriptor: String) -> Result<Balance, WalletError> {
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

    //use crate::{Config, Wallet};
    //use bdk::bitcoin::Network;

    //const WATCH_DESCRIPTOR: &str = "wpkh([aed2a027/84'/1'/0']tpubDCvyR4gGk5U6r1Q1HMQtgZYMD3a9bVyt7Tv9BWgcBCQsff4aqR7arUGPTMaUbVwaH8TeaK924GJr9nHyGPBtqSCD8BCjMnJb1qZFjK4ACfL/0/*)";

    /*
    #[test]
    fn test_get_balance() {
        let wallet = Wallet::new(Config {
            electrum_url: "ssl://electrum.blockstream.info:60002".to_string(),
            network: Network::Testnet,
        })
        .unwrap();

        let balance = wallet.get_balance(WATCH_DESCRIPTOR.to_string()).unwrap();

        assert_eq!(balance.confirmed, 88009);
    }*/
}
