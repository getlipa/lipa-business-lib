use crate::secrets::KeyPair;
use crate::signing::sign;
use graphql_client::reqwest::post_graphql_blocking;
use graphql_client::GraphQLQuery;
use reqwest::blocking::Client;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct RequestChallenge;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct StartSession;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct PrepareWalletSession;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct UnlockWallet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct RefreshSession;

#[allow(non_camel_case_types)]
type timestamptz = u64;
#[allow(non_camel_case_types)]
type uuid = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema_wallet_read.graphql",
    query_path = "src/operations.graphql",
    response_derives = "Debug"
)]
pub struct GetBusinessOwner;

const GRAPHQL_API_URL: &str = "https://api-test.getlipa.com/v1/graphql";

pub enum AuthLevel {
    Basic,
    Owner,
    Employee,
}

pub(crate) struct AuthProvider {
    client: Client,
    auth_level: AuthLevel,
    wallet_keypair: KeyPair,
    auth_keypair: KeyPair,
    refresh_token: Option<String>,
}

impl AuthProvider {
    pub fn new(auth_level: AuthLevel, wallet_keypair: KeyPair, auth_keypair: KeyPair) -> Self {
        let client = Client::builder()
            .user_agent("graphql-rust/0.11.0")
            .build()
            .unwrap();
        AuthProvider {
            client,
            auth_level,
            wallet_keypair,
            auth_keypair,
            refresh_token: None,
        }
    }

    pub fn query_token(&mut self) -> String {
        let (access_token, refresh_token) = match self.refresh_token.take() {
            Some(refresh_token) => {
                // TODO: Tolerate invalid refresh token error and fallback to
                // run_aut_flow().
                self.refresh_session(refresh_token)
            }
            None => self.run_auth_flow(),
        };
        self.refresh_token = Some(refresh_token);
        access_token
    }

    fn run_auth_flow(&self) -> (String, String) {
        let (access_token, refresh_token, wallet_pub_key_id) = self.start_basic_session();

        match self.auth_level {
            AuthLevel::Basic => (access_token, refresh_token),
            AuthLevel::Owner => self.start_priviledged_session(access_token, wallet_pub_key_id),
            AuthLevel::Employee => {
                let owner_pub_key_id =
                    self.get_business_owner(access_token.clone(), wallet_pub_key_id);
                if let Some(owner_pub_key_id) = owner_pub_key_id {
                    self.start_priviledged_session(access_token, owner_pub_key_id)
                } else {
                    panic!("Employee does not belong to any owner");
                }
            }
        }
    }

    fn start_basic_session(&self) -> (String, String, String) {
        let challenge = self.request_challenge();

        let challenge_with_prefix = add_bitcoin_message_prefix(&challenge);
        let challenge_signature = sign(challenge_with_prefix, self.auth_keypair.secret_key.clone());

        let auth_pub_key_with_prefix = add_hex_prefix(&self.auth_keypair.public_key);
        let signed_auth_pub_key = sign(
            auth_pub_key_with_prefix,
            self.wallet_keypair.secret_key.clone(),
        );

        println!("Starting session ...");
        let variables = start_session::Variables {
            auth_pub_key: add_hex_prefix(&self.auth_keypair.public_key),
            challenge,
            challenge_signature: add_hex_prefix(&challenge_signature),
            wallet_pub_key: add_hex_prefix(&self.wallet_keypair.public_key),
            signed_auth_pub_key: add_hex_prefix(&signed_auth_pub_key),
        };

        let response_body =
            post_graphql_blocking::<StartSession, _>(&self.client, GRAPHQL_API_URL, variables)
                .unwrap();
        // println!("Response body: {:?}", response_body);
        let session_permit = response_body.data.unwrap().start_session_v2.unwrap();
        let access_token = session_permit.access_token.unwrap();
        let refresh_token = session_permit.refresh_token.unwrap();
        let wallet_pub_key_id = session_permit.wallet_pub_key_id.unwrap();
        println!("access_token: {}", access_token);
        println!("refresh_token: {}", refresh_token);
        println!("wallet_pub_key_id: {}", wallet_pub_key_id);
        (access_token, refresh_token, wallet_pub_key_id)
    }

    fn start_priviledged_session(
        &self,
        access_token: String,
        owner_pub_key_id: String,
    ) -> (String, String) {
        let challenge = self.request_challenge();

        let challenge_with_prefix = add_bitcoin_message_prefix(&challenge);
        let challenge_signature = sign(
            challenge_with_prefix,
            self.wallet_keypair.secret_key.clone(),
        );

        println!("Preparing wallet session ...");
        let variables = prepare_wallet_session::Variables {
            wallet_pub_key_id: owner_pub_key_id,
            challenge: challenge.clone(),
            signed_challenge: add_hex_prefix(&challenge_signature),
        };

        let client_with_token = Client::builder()
            .user_agent("graphql-rust/0.11.0")
            .default_headers(
                std::iter::once((
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", access_token))
                        .unwrap(),
                ))
                .collect(),
            )
            .build()
            .unwrap();
        let response_body = post_graphql_blocking::<PrepareWalletSession, _>(
            &client_with_token,
            GRAPHQL_API_URL,
            variables,
        )
        .unwrap();
        // println!("Response body: {:?}", response_body);
        let prepared_permission_token = response_body.data.unwrap().prepare_wallet_session.unwrap();

        println!("Starting wallet session ...");
        let variables = unlock_wallet::Variables {
            challenge,
            challenge_signature: add_hex_prefix(&challenge_signature),
            prepared_permission_token,
        };
        let response_body = post_graphql_blocking::<UnlockWallet, _>(
            &client_with_token,
            GRAPHQL_API_URL,
            variables,
        )
        .unwrap();
        // println!("Response body: {:?}", response_body);
        let session_permit = response_body.data.unwrap().start_prepared_session.unwrap();
        let access_token = session_permit.access_token.unwrap();
        let refresh_token = session_permit.refresh_token.unwrap();

        println!("access_token: {}", access_token);
        println!("refresh_token: {}", refresh_token);

        (access_token, refresh_token)
    }

    fn get_business_owner(
        &self,
        access_token: String,
        wallet_pub_key_id: String,
    ) -> Option<String> {
        println!("Getting business owner ...");
        let client_with_token = Client::builder()
            .user_agent("graphql-rust/0.11.0")
            .default_headers(
                std::iter::once((
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", access_token))
                        .unwrap(),
                ))
                .collect(),
            )
            .build()
            .unwrap();
        let variables = get_business_owner::Variables {
            owner_wallet_pub_key_id: wallet_pub_key_id,
        };
        let response_body = post_graphql_blocking::<GetBusinessOwner, _>(
            &client_with_token,
            GRAPHQL_API_URL,
            variables,
        )
        .unwrap();
        // println!("Response body: {:?}", response_body);
        let result = response_body
            .data
            .unwrap()
            .wallet_acl
            .first()
            .map(|w| w.owner_wallet_pub_key_id.clone());
        println!("Owner: {:?}", result);
        result
    }

    fn refresh_session(&self, refresh_token: String) -> (String, String) {
        // Refresh session.
        println!("Refreshing session ...");
        let variables = refresh_session::Variables { refresh_token };
        let response_body =
            post_graphql_blocking::<RefreshSession, _>(&self.client, GRAPHQL_API_URL, variables)
                .unwrap();
        // println!("Response body: {:?}", response_body);
        let session_permit = response_body.data.unwrap().refresh_session.unwrap();
        let access_token = session_permit.access_token.unwrap();
        let refresh_token = session_permit.refresh_token.unwrap();

        println!("access_token: {}", access_token);
        println!("refresh_token: {}", refresh_token);

        (access_token, refresh_token)
    }

    fn request_challenge(&self) -> String {
        println!("Requesting challenge ...");
        let variables = request_challenge::Variables {};
        let response_body =
            post_graphql_blocking::<RequestChallenge, _>(&self.client, GRAPHQL_API_URL, variables)
                .unwrap();
        let response_data: request_challenge::ResponseData = response_body.data.unwrap();
        response_data.auth_challenge.unwrap()
    }
}

fn add_hex_prefix(string: &str) -> String {
    ["\\x", string].concat()
}

fn add_bitcoin_message_prefix(string: &str) -> String {
    ["\\x18Bitcoin Signed Message:", string].concat()
}
