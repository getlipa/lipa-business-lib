use crate::KeyPair;
use honey_badger::errors::Result;
use honey_badger::AuthLevel;

pub struct Auth {
    auth: honey_badger::Auth,
}

impl Auth {
    pub fn new(
        backend_url: String,
        auth_level: AuthLevel,
        wallet_keypair: KeyPair,
        auth_keypair: KeyPair,
    ) -> Result<Self> {
        let wallet_keypair = honey_badger::secrets::KeyPair {
            secret_key: wallet_keypair.secret_key,
            public_key: wallet_keypair.public_key,
        };
        let auth_keypair = honey_badger::secrets::KeyPair {
            secret_key: auth_keypair.secret_key,
            public_key: auth_keypair.public_key,
        };
        Ok(Auth {
            auth: honey_badger::Auth::new(backend_url, auth_level, wallet_keypair, auth_keypair)?,
        })
    }

    pub fn query_token(&self) -> Result<String> {
        self.auth.query_token()
    }

    pub fn get_wallet_pubkey_id(&self) -> Option<String> {
        self.auth.get_wallet_pubkey_id()
    }
}
