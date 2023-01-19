use crate::errors::AuthResult;

use base64::{engine::general_purpose, Engine as _};
use lipa_errors::{MapToLipaError, OptionToError};
use serde_json::Value;
use std::time::{Duration, SystemTime};

pub(crate) struct Token {
    pub raw: String,
    pub received_at: SystemTime,
    pub expires_at: SystemTime,
}

pub(crate) fn parse_token(raw_token: String) -> AuthResult<Token> {
    let splitted_jwt_strings: Vec<_> = raw_token.split('.').collect();

    let jwt_body = splitted_jwt_strings.get(1).ok_or_invalid_input(
        "Failed to get JWT body: JWT String isn't split with '.' characters",
    )?;

    let decoded_jwt_body = general_purpose::STANDARD_NO_PAD
        .decode(jwt_body)
        .map_to_invalid_input("Failed to decode JWT")?;
    let converted_jwt_body = String::from_utf8(decoded_jwt_body)
        .map_to_invalid_input("Failed to decode serialized JWT into json")?;

    let parsed_jwt_body = serde_json::from_str::<Value>(&converted_jwt_body)
        .map_to_invalid_input("Failed to get parse JWT json")?;

    let received_at = SystemTime::now();
    let expires_at = get_expiry(&parsed_jwt_body)?;

    Ok(Token {
        raw: raw_token,
        received_at,
        expires_at,
    })
}

fn get_expiry(jwt_body: &Value) -> AuthResult<SystemTime> {
    let expiry = jwt_body
        .as_object()
        .ok_or_invalid_input("Failed to get JWT body json object")?
        .get("exp")
        .ok_or_invalid_input("JWT doesn't have an expiry field")?
        .as_u64()
        .ok_or_invalid_input("Failed to parse JWT expiry into unsigned integer")?;

    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(expiry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jwt() {
        let raw = "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJodHRwczovL2hhc3VyYS5pby9qd3QvY2xhaW1zIjp7IngtaGFzdXJhLWRlZmF1bHQtcm9sZSI6IldBTExFVF9SRUFEIiwieC1oYXN1cmEtc2Vzc2lvbi1pZCI6IjAwNGY1ODU4LTMzMTItNDNiNS04YzY1LWZlYThjODViZWQzNSIsIngtaGFzdXJhLXdhbGxldC1wdWIta2V5LWlkIjoiMmIzYjNmMzQtMWMwNC01ZjZkLTlmZDUtYWU4OWE5YzQ1NGJlIiwieC1oYXN1cmEtd2FsbGV0LXB1Yi1rZXkiOiJcXHgwMzkzZGE1ODQwZTBjYjZjOGI5ZWIzNWQyMzJhODgzYWFkNTJkNzI1ZjUxMjY0NTVjN2Y2OWMzNjgzZDBhMGMyODYiLCJ4LWhhc3VyYS1hbGxvd2VkLXJvbGVzIjpbIldBTExFVF9SRUFEIl19LCJpc3MiOiJnZXRsaXBhLmNvbSIsImV4cCI6MTY3NDEzODI1NH0.TWZBJfPHhDRIw7ZVRM9SKhRgTQrvGYwZMWIBS0Nd0r5n81LuJb_u3WovrkZr_F62aH4cep8a1FDqF1eHzpLfuXOqJfeCiMAP8jGHZJDz9zyimyN7ZYnElUSRHWTACL2p0RoszjRcHdQcuKfBhvpsqR0JbmhiAyT-h8rWiOOBa9akuFyJO-5C8jFT1UxTp9eEE89MEsMaINAttlBHyaywwMLvtN52LbwcfhYA961-xV7jwRaf29p6Q_ewC07tQnrCOnyPkTXNpvSG-djF-9ETeDM75k5iDU48EqljtT2wdR46ysIBWOCquuRd5eOzOCvXo0yvrGKPJpRo2JcpEBr_uQ".to_string();
        let token = parse_token(raw).unwrap();
        let exp = token
            .expires_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(exp, 1674138254);
    }
}
