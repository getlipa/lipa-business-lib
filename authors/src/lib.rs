pub mod provider;
pub mod secrets;
mod signing;

pub use provider::AuthLevel;

use provider::AuthProvider;

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::secrets::KeyPair;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

const TOKEN_TO_BE_YET_VALID: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct Token {
    raw: String,
    expires_at: SystemTime,
}

pub struct Auth {
    provider: Mutex<AuthProvider>,
    token: Mutex<Token>,
}

impl Auth {
    pub fn new(auth_level: AuthLevel, wallet_keypair: KeyPair, auth_keypair: KeyPair) -> Self {
        let mut provider = AuthProvider::new(auth_level, wallet_keypair, auth_keypair);
        let token = parse_token(provider.query_token());
        Auth {
            provider: Mutex::new(provider),
            token: Mutex::new(token),
        }
    }

    pub fn query_token(&self) -> String {
        if let Some(token) = self.get_token_if_valid() {
            return token;
        }

        let mut provider = self.provider.lock().unwrap();
        // Anyone else refreshed the token by chance?...
        if let Some(token) = self.get_token_if_valid() {
            return token;
        }

        let token = parse_token(provider.query_token());
        *self.token.lock().unwrap() = token;
        if let Some(token) = self.get_token_if_valid() {
            return token;
        }
        panic!("Newly refreshed token is not valid long enough");
    }

    fn get_token_if_valid(&self) -> Option<String> {
        let now = SystemTime::now();
        let token = self.token.lock().unwrap();
        // TODO: Substruct 10% of token validity.
        if token.expires_at > now + TOKEN_TO_BE_YET_VALID {
            Some(token.raw.clone())
        } else {
            None
        }
    }
}

fn parse_token(raw_token: String) -> Token {
    let splitted_jwt_strings: Vec<_> = raw_token.split('.').collect();

    let jwt_body = splitted_jwt_strings.get(1).unwrap();

    let decoded_jwt_body = general_purpose::STANDARD_NO_PAD.decode(jwt_body).unwrap();
    let converted_jwt_body = String::from_utf8(decoded_jwt_body).unwrap();

    let parsed_jwt_body = serde_json::from_str::<serde_json::Value>(&converted_jwt_body).unwrap();

    let expires_at = get_expiry(&parsed_jwt_body);

    println!(
        "The parsed token will expiry in {} secs",
        expires_at
            .duration_since(SystemTime::now())
            .unwrap()
            .as_secs()
    );
    Token {
        raw: raw_token,
        expires_at,
    }
}

fn get_expiry(jwt_body: &Value) -> SystemTime {
    let expiry = jwt_body
        .as_object()
        .unwrap()
        .get("exp")
        .unwrap()
        .as_u64()
        .unwrap();

    SystemTime::UNIX_EPOCH + Duration::from_secs(expiry)
}
