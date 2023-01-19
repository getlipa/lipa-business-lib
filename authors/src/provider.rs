use crate::errors::{AuthError, AuthResult, AuthRuntimeErrorCode};
use crate::graphql::*;
use crate::secrets::KeyPair;
use crate::signing::sign;

use graphql_client::reqwest::post_graphql_blocking;
use graphql_client::Response;
use lipa_errors::{permanent_failure, runtime_error, MapToLipaError, OptionToError};
use log::{info, trace};
use reqwest::blocking::Client;

const AUTH_EXCEPTION_CODE: &str = "authentication-exception";
const INVALID_JWT_ERROR_CODE: &str = "invalid-jwt";
const MISSING_HTTP_HEADER_EXCEPTION_CODE: &str = "http-header-missing-exception";
const INVALID_INVITATION_EXCEPTION_CODE: &str = "invalid-invitation-exception";
const REMOTE_SCHEMA_ERROR_CODE: &str = "remote-schema-error";

pub enum AuthLevel {
    Basic,
    Owner,
    Employee,
}

pub(crate) struct AuthProvider {
    backend_url: String,
    client: Client,
    auth_level: AuthLevel,
    wallet_keypair: KeyPair,
    auth_keypair: KeyPair,
    refresh_token: Option<String>,
}

impl AuthProvider {
    pub fn new(
        backend_url: String,
        auth_level: AuthLevel,
        wallet_keypair: KeyPair,
        auth_keypair: KeyPair,
    ) -> AuthResult<Self> {
        let client = Client::builder()
            .user_agent("graphql-rust/0.11.0")
            .build()
            .map_to_permanent_failure("Failed to build a reqwest client")?;
        Ok(AuthProvider {
            backend_url,
            client,
            auth_level,
            wallet_keypair,
            auth_keypair,
            refresh_token: None,
        })
    }

    pub fn query_token(&mut self) -> AuthResult<String> {
        let (access_token, refresh_token) = match self.refresh_token.clone() {
            Some(refresh_token) => {
                match self.refresh_session(refresh_token) {
                    // Tolerate authentication errors and retry auth flow.
                    Err(AuthError::RuntimeError {
                        code: AuthRuntimeErrorCode::AuthServiceError,
                        ..
                    }) => self.run_auth_flow(),
                    result => result,
                }
            }
            None => self.run_auth_flow(),
        }?;
        self.refresh_token = Some(refresh_token);
        Ok(access_token)
    }

    fn run_auth_flow(&self) -> AuthResult<(String, String)> {
        let (access_token, refresh_token, wallet_pub_key_id) = self.start_basic_session()?;

        match self.auth_level {
            AuthLevel::Basic => Ok((access_token, refresh_token)),
            AuthLevel::Owner => self.start_priviledged_session(access_token, wallet_pub_key_id),
            AuthLevel::Employee => {
                let owner_pub_key_id =
                    self.get_business_owner(access_token.clone(), wallet_pub_key_id)?;
                if let Some(owner_pub_key_id) = owner_pub_key_id {
                    self.start_priviledged_session(access_token, owner_pub_key_id)
                } else {
                    panic!("Employee does not belong to any owner");
                }
            }
        }
    }

    fn start_basic_session(&self) -> AuthResult<(String, String, String)> {
        let challenge = self.request_challenge()?;

        let challenge_with_prefix = add_bitcoin_message_prefix(&challenge);
        let challenge_signature = sign(challenge_with_prefix, self.auth_keypair.secret_key.clone());

        let auth_pub_key_with_prefix = add_hex_prefix(&self.auth_keypair.public_key);
        let signed_auth_pub_key = sign(
            auth_pub_key_with_prefix,
            self.wallet_keypair.secret_key.clone(),
        );

        info!("Starting session ...");
        let variables = start_session::Variables {
            auth_pub_key: add_hex_prefix(&self.auth_keypair.public_key),
            challenge,
            challenge_signature: add_hex_prefix(&challenge_signature),
            wallet_pub_key: add_hex_prefix(&self.wallet_keypair.public_key),
            signed_auth_pub_key: add_hex_prefix(&signed_auth_pub_key),
        };

        let response_body =
            post_graphql_blocking::<StartSession, _>(&self.client, &self.backend_url, variables)
                .map_to_runtime_error(
                    AuthRuntimeErrorCode::NetworkError,
                    "Failed to get a response to a start_session request",
                )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let session_permit = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .start_session_v2
            .ok_or_permanent_failure(
                "Response to start_session request doesn't have the expected structure",
            )?;
        let access_token = session_permit.access_token.ok_or_permanent_failure(
            "Response to start_session request doesn't have the expected structure: missing access token",
        )?;
        let refresh_token = session_permit.refresh_token.ok_or_permanent_failure(
            "Response to start_session request doesn't have the expected structure: missing refresh token",
        )?;
        let wallet_pub_key_id = session_permit.wallet_pub_key_id.ok_or_permanent_failure(
            "Response to start_session request doesn't have the expected structure: missing wallet public key id",
        )?;
        info!("access_token: {}", access_token);
        info!("refresh_token: {}", refresh_token);
        info!("wallet_pub_key_id: {}", wallet_pub_key_id);
        Ok((access_token, refresh_token, wallet_pub_key_id))
    }

    fn start_priviledged_session(
        &self,
        access_token: String,
        owner_pub_key_id: String,
    ) -> AuthResult<(String, String)> {
        let challenge = self.request_challenge()?;

        let challenge_with_prefix = add_bitcoin_message_prefix(&challenge);
        let challenge_signature = sign(
            challenge_with_prefix,
            self.wallet_keypair.secret_key.clone(),
        );

        info!("Preparing wallet session ...");
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
                        .map_to_permanent_failure("Failed to build header value from str")?,
                ))
                .collect(),
            )
            .build()
            .map_to_permanent_failure("Failed to build a reqwest client")?;
        let response_body = post_graphql_blocking::<PrepareWalletSession, _>(
            &client_with_token,
            &self.backend_url,
            variables,
        )
        .map_to_runtime_error(
            AuthRuntimeErrorCode::NetworkError,
            "Failed to get a response to a prepare_wallet_session request",
        )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let prepared_permission_token = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .prepare_wallet_session
            .ok_or_permanent_failure(
                "Response to prepare_wallet_session request doesn't have the expected structure",
            )?;

        info!("Starting wallet session ...");
        let variables = unlock_wallet::Variables {
            challenge,
            challenge_signature: add_hex_prefix(&challenge_signature),
            prepared_permission_token,
        };
        let response_body = post_graphql_blocking::<UnlockWallet, _>(
            &client_with_token,
            &self.backend_url,
            variables,
        )
        .map_to_runtime_error(
            AuthRuntimeErrorCode::NetworkError,
            "Failed to get a response to a unlock_wallet request",
        )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let session_permit = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .start_prepared_session
            .ok_or_permanent_failure(
                "Response to unlock_wallet request doesn't have the expected structure",
            )?;
        let access_token = session_permit.access_token.ok_or_permanent_failure(
            "Response to unlock_wallet request doesn't have the expected structure: missing access token",
        )?;
        let refresh_token = session_permit.refresh_token.ok_or_permanent_failure(
            "Response to unlock_wallet request doesn't have the expected structure: missing refresh token",
        )?;

        info!("access_token: {}", access_token);
        info!("refresh_token: {}", refresh_token);

        Ok((access_token, refresh_token))
    }

    fn get_business_owner(
        &self,
        access_token: String,
        wallet_pub_key_id: String,
    ) -> AuthResult<Option<String>> {
        info!("Getting business owner ...");
        let client_with_token = Client::builder()
            .user_agent("graphql-rust/0.11.0")
            .default_headers(
                std::iter::once((
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", access_token))
                        .map_to_permanent_failure("Failed to build header value from str")?,
                ))
                .collect(),
            )
            .build()
            .map_to_permanent_failure("Failed to build a reqwest client")?;
        let variables = get_business_owner::Variables {
            owner_wallet_pub_key_id: wallet_pub_key_id,
        };
        let response_body = post_graphql_blocking::<GetBusinessOwner, _>(
            &client_with_token,
            &self.backend_url,
            variables,
        )
        .map_to_runtime_error(
            AuthRuntimeErrorCode::NetworkError,
            "Failed to get a response to a get_business_owner request",
        )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let result = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .wallet_acl
            .first()
            .map(|w| w.owner_wallet_pub_key_id.clone());
        info!("Owner: {:?}", result);
        Ok(result)
    }

    fn refresh_session(&self, refresh_token: String) -> AuthResult<(String, String)> {
        // Refresh session.
        info!("Refreshing session ...");
        let variables = refresh_session::Variables { refresh_token };
        let response_body =
            post_graphql_blocking::<RefreshSession, _>(&self.client, &self.backend_url, variables)
                .map_to_runtime_error(
                    AuthRuntimeErrorCode::NetworkError,
                    "Failed to get a response to a refresh_session request",
                )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let session_permit = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .refresh_session
            .ok_or_permanent_failure(
                "Response to refresh_session request doesn't have the expected structure",
            )?;
        let access_token = session_permit.access_token.ok_or_permanent_failure(
            "Response to unlock_wallet request doesn't have the expected structure: missing access token",
        )?;
        let refresh_token = session_permit.refresh_token.ok_or_permanent_failure(
            "Response to unlock_wallet request doesn't have the expected structure: missing refresh token",
        )?;

        info!("access_token: {}", access_token);
        info!("refresh_token: {}", refresh_token);

        Ok((access_token, refresh_token))
    }

    fn request_challenge(&self) -> AuthResult<String> {
        info!("Requesting challenge ...");
        let variables = request_challenge::Variables {};
        let response_body = post_graphql_blocking::<RequestChallenge, _>(
            &self.client,
            &self.backend_url,
            variables,
        )
        .map_to_runtime_error(
            AuthRuntimeErrorCode::NetworkError,
            "Failed to get a response to a request_challenge request",
        )?;
        trace!("Response body: {:?}", response_body);

        Self::handle_response_errors(&response_body)?;

        let challenge = response_body
            .data
            .ok_or_permanent_failure("Response has no data")?
            .auth_challenge
            .ok_or_permanent_failure(
                "Response to request_challenge request doesn't have the expected structure: missing auth challenge",
            )?;

        Ok(challenge)
    }

    fn handle_response_errors<D>(response: &Response<D>) -> AuthResult<()> {
        if let Some(errors) = response.errors.as_ref() {
            let error = errors
                .get(0)
                .ok_or_permanent_failure("Unexpected backend response: errors empty")?;
            let code = error
                .extensions
                .as_ref()
                .ok_or_permanent_failure("Unexpected backend response: error without extensions")?
                .get("code")
                .ok_or_permanent_failure("Unexpected backend response: error without code")?
                .as_str()
                .ok_or_permanent_failure("Unexpected backend response: error code isn't string")?;

            match code {
                AUTH_EXCEPTION_CODE => {
                    Err(runtime_error(
                        AuthRuntimeErrorCode::AuthServiceError,
                        "The backend threw an Authentication Exception",
                    ))
                }
                INVALID_JWT_ERROR_CODE => {
                    Err(runtime_error(
                        AuthRuntimeErrorCode::AuthServiceError,
                        "A request we made included an invalid JWT"
                    ))
                }
                MISSING_HTTP_HEADER_EXCEPTION_CODE => {
                    Err(permanent_failure(
                        "A request we made didn't include the necessary HTTP header",
                    ))
                }
                INVALID_INVITATION_EXCEPTION_CODE => {
                    Err(permanent_failure(
                        "Unexpected backend response: invalid invitation when no invitations have been made"
                    ))
                },
                REMOTE_SCHEMA_ERROR_CODE => {
                    Err(permanent_failure("A remote schema call has failed on the backend"))
                }
                _ => {
                    Err(permanent_failure(
                        format!("Unexpected backend response: unknown error code {}", code),
                    ))
                }
            }
        } else {
            Ok(())
        }
    }
}

fn add_hex_prefix(string: &str) -> String {
    ["\\x", string].concat()
}

fn add_bitcoin_message_prefix(string: &str) -> String {
    ["\\x18Bitcoin Signed Message:", string].concat()
}
