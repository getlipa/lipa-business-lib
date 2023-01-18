use crate::KeyPair;
use authors::errors::AuthResult;
use authors::AuthLevel;

pub struct Auth {
    auth: authors::Auth,
}

impl Auth {
    pub fn new(
        backend_url: String,
        auth_level: AuthLevel,
        wallet_keypair: KeyPair,
        auth_keypair: KeyPair,
    ) -> AuthResult<Self> {
        let wallet_keypair = authors::secrets::KeyPair {
            secret_key: wallet_keypair.secret_key,
            public_key: wallet_keypair.public_key,
        };
        let auth_keypair = authors::secrets::KeyPair {
            secret_key: auth_keypair.secret_key,
            public_key: auth_keypair.public_key,
        };
        Ok(Auth {
            auth: authors::Auth::new(backend_url, auth_level, wallet_keypair, auth_keypair)?,
        })
    }

    pub fn query_token(&self) -> AuthResult<String> {
        self.auth.query_token()
    }
}
