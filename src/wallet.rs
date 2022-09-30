use crate::errors::WalletError;
use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::{Balance, SyncOptions};

pub struct Config {
    pub electrum_url: String,
    pub network: Network,
    pub watch_descriptor: String,
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

    pub fn get_balance(&self) -> Result<Balance, WalletError> {
        let wallet = bdk::Wallet::new(
            &self.config.watch_descriptor,
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
