use crate::KeyPair;
use authors::AuthLevel;

pub struct Auth {
    auth: authors::Auth,
}

impl Auth {
    pub fn new(auth_level: AuthLevel, wallet_keypair: KeyPair, auth_keypair: KeyPair) -> Self {
        let wallet_keypair = authors::secrets::KeyPair {
            secret_key: wallet_keypair.secret_key,
            public_key: wallet_keypair.public_key,
        };
        let auth_keypair = authors::secrets::KeyPair {
            secret_key: auth_keypair.secret_key,
            public_key: auth_keypair.public_key,
        };
        Auth {
            auth: authors::Auth::new(auth_level, wallet_keypair, auth_keypair),
        }
    }

    pub fn query_token(&self) -> String {
        self.auth.query_token()
    }
}
