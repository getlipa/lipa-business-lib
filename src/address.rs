use bdk::bitcoin::{Address, Network};
use bip21::Uri;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AddressParsingError {
    #[error("Invalid network: expected {expected}, but address is for {address}")]
    InvalidNetwork { expected: Network, address: Network },
    #[error("Other")]
    Other,
}

pub fn parse_address(
    address: String,
    expected_network: Network,
) -> Result<Address, AddressParsingError> {
    let bip21_prefix = "bitcoin:";

    let address = from_qr_uri(address);

    let address = if address.starts_with(bip21_prefix) {
        let result: Result<bip21::Uri<'_>, bip21::de::Error<_>> = Uri::from_str(&address);
        match result {
            Ok(uri) => Ok(uri.address),
            Err(_) => Err(AddressParsingError::Other),
        }
    } else {
        Address::from_str(&address).map_err(|_| AddressParsingError::Other)
    }?;

    if address.is_valid_for_network(expected_network) {
        Ok(address)
    } else {
        Err(AddressParsingError::InvalidNetwork {
            expected: expected_network,
            address: address.network,
        })
    }
}

fn from_qr_uri(address: String) -> String {
    if address.starts_with("BITCOIN:") {
        address.to_lowercase()
    } else {
        address
    }
}

#[cfg(test)]
mod tests {
    use crate::address::{parse_address, AddressParsingError};
    use bdk::bitcoin::Network;

    const MAINNET: Network = Network::Bitcoin;
    const TESTNET: Network = Network::Testnet;

    #[test]
    fn valid_mainnet() {
        let p2pkh = "151111ZKuNi4r9Ker4PjTMR1hf9TdwKe6W".to_string();
        let result = parse_address(p2pkh.clone(), MAINNET);
        assert_eq!(result.unwrap().to_string(), p2pkh);

        let p2sh = "351112e6qVY9zzZ5HZGxhcYnX975AVzYxt".to_string();
        let result = parse_address(p2sh.clone(), MAINNET);
        assert_eq!(result.unwrap().to_string(), p2sh);

        let p2wpkh = "bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw".to_string();
        let result = parse_address(p2wpkh.clone(), MAINNET);
        assert_eq!(result.unwrap().to_string(), p2wpkh);

        let p2tr = "bc1p0000awrdl80vv4j8tmx82sfxd58jl9mmln9wshqynk8sv9g9et3qzdpkkq".to_string();
        let result = parse_address(p2tr.clone(), MAINNET);
        assert_eq!(result.unwrap().to_string(), p2tr);
    }

    #[test]
    fn valid_testnet() {
        let p2pkh = "mqLMuMmLKHKfMExHVaUB7qcmhULSPAmdpH".to_string();
        let result = parse_address(p2pkh.clone(), TESTNET);
        assert_eq!(result.unwrap().to_string(), p2pkh);

        let p2sh = "2N6cWfrWV9Kepj9vuFGQGzjoF96QtKnYY1P".to_string();
        let result = parse_address(p2sh.clone(), TESTNET);
        assert_eq!(result.unwrap().to_string(), p2sh);

        let p2wpkh = "tb1q00000alt56z8fsczc67u7q0vsl0wrqt52x084l".to_string();
        let result = parse_address(p2wpkh.clone(), TESTNET);
        assert_eq!(result.unwrap().to_string(), p2wpkh);

        let p2tr = "tb1p67fy6nmag04fvkjxtt3sjhl5zyc7t9r08jzl08jy4k703cn7pq8q39zmvg".to_string();
        let result = parse_address(p2tr.clone(), TESTNET);
        assert_eq!(result.unwrap().to_string(), p2tr);
    }
    #[test]
    fn valid_mainnet_bip21() {
        let mainnet_p2wpkh =
            "bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw".to_string();

        let mainnet_p2wpkh_bip21 =
            "bitcoin:bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw".to_string();
        let result = parse_address(mainnet_p2wpkh_bip21, MAINNET);
        assert_eq!(result.unwrap().to_string(), mainnet_p2wpkh);

        let mainnet_p2wpkh_bip21 =
            "BITCOIN:BC1QHZTYDHU3P30H0LD5CRUCMMDRSPP2XJTG8XR3F32708AL70EEGH7QAQ50YW".to_string();
        let result = parse_address(mainnet_p2wpkh_bip21, MAINNET);
        assert_eq!(result.unwrap().to_string(), mainnet_p2wpkh);

        let mainnet_p2wpkh_bip21_with_params =
            "bitcoin:bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw?amount=0.000001&label=gude%20von%20Onleines%20&message=gude%20von%20Onleines%20".to_string();
        let result = parse_address(mainnet_p2wpkh_bip21_with_params, MAINNET);
        assert_eq!(result.unwrap().to_string(), mainnet_p2wpkh);

        let mainnet_p2wpkh_bip21_with_params =
            "bitcoin:bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw?amount=0.00000111&lightning=LNBC1110N1P3UHH2KDQQNP4QF9N63RP8AH4GUJ5PUXUHFWQPWA9RC4QYF4VC0QQ432MQ3H9NK6GXPP5VYFZ03QT23J8TQP0LQH8AQ3WZ7DHYUDRV0Y2KLFKTNCHAK40PWHSSP5JJXD08RDQJ2TDGN3MTHX69K8987Z8N4ZPSQ0NQL89XXGXCQVE0DQ9QYYSGQCQPCXQRRSSRZJQ2TT9KE59L8C0655MXQH2L7LF5L9GK74EM6FR86CKHFCMLWH806UJZ72CCQQKTGQQQQQQQQQQQQQQQGQ9Q5GECTCYW7CK998RDFWW0LDGDXP974S0XS6YKLZ2DJ0URRFK2QSE8WLETS3AVYAVAAE2TAM99LVCQHUXKX3T78GPPDJA8DPJGZF0H8PGP57Q0AF".to_string();
        let result = parse_address(mainnet_p2wpkh_bip21_with_params, MAINNET);
        assert_eq!(result.unwrap().to_string(), mainnet_p2wpkh);
    }

    #[test]
    fn invalid_network() {
        let mainnet_p2wpkh =
            "bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw".to_string();
        let result = parse_address(mainnet_p2wpkh, TESTNET);
        assert!(matches!(
            result,
            Err(AddressParsingError::InvalidNetwork {
                expected: TESTNET,
                address: MAINNET,
            })
        ));

        let mainnet_p2wpkh =
            "bc1qhztydhu3p30h0ld5crucmmdrspp2xjtg8xr3f32708al70eegh7qaq50yw".to_string();
        let result = parse_address(mainnet_p2wpkh, TESTNET);
        assert!(matches!(
            result,
            Err(AddressParsingError::InvalidNetwork {
                expected: TESTNET,
                address: MAINNET,
            })
        ));
    }

    #[test]
    fn invalid_address() {
        let result = parse_address("invalid".to_string(), Network::Regtest);
        assert!(matches!(result, Err(AddressParsingError::Other)));

        let ln_invoice = "lnbc15u1p3xnhl2pp5jptserfk3zk4qy42tlucycrfwxhydvlemu9pqr93tuzlv9cc7g3sdqsvfhkcap3xyhx7un8cqzpgxqzjcsp5f8c52y2stc300gl6s4xswtjpc37hrnnr3c9wvtgjfuvqmpm35evq9qyyssqy4lgd8tj637qcjp05rdpxxykjenthxftej7a2zzmwrmrl70fyj9hvj0rewhzj7jfyuwkwcg9g2jpwtk3wkjtwnkdks84hsnu8xps5vsq4gj5hs".to_string();
        let result = parse_address(ln_invoice, Network::Signet);
        assert!(matches!(result, Err(AddressParsingError::Other)));
    }
}
