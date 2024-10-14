use reqwest::Client;
use reqwest::header::HeaderMap;
use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, DecodePublicKey};
use serde::Deserialize;
use crate::utils::constants::URL;

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub private_key: RsaPrivateKey,
    pub public_key: RsaPublicKey,
    /// Private key in PKCS#8 format
    pub private_key_string: String,
    /// Public key in PKCS#8 format
    pub public_key_string: String,
}

/// Takes key_size (in bits) and returns an RSA KeyPair
pub async fn generate_key_pair(key_size: usize) -> Result<KeyPair, String> {
    let mut rng = rand::thread_rng();
    match RsaPrivateKey::new(&mut rng, key_size) {
        Ok(private_key) => {
            let public_key = RsaPublicKey::from(&private_key);

            let private_key_string = private_key.to_pkcs8_pem(Default::default());
            let public_key_string = public_key.to_public_key_pem(Default::default());

            if private_key_string.is_ok() && public_key_string.is_ok() {
                let private_key_string = private_key_string.unwrap().to_string();
                let public_key_string = public_key_string.unwrap();

                Ok(KeyPair { private_key, public_key, private_key_string, public_key_string })
            } else {
                Err("Failed to convert private key and/or public key to pkcs8 pem!".to_string())
            }
        }
        Err(e) => {
            Err(format!("Failed to generate Private key!: {}", e))
        }
    }
}

pub(crate) async fn handshake(client: &Client) -> Result<(), String> {
    
    Ok(())
}

pub(crate) async fn get_public_key(client: &Client) -> Result<RsaPublicKey, String> {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", "*/*".parse().unwrap());
    headers.insert("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8".parse().unwrap());
    headers.insert("Sec-Fetch-Dest", "empty".parse().unwrap());
    headers.insert("Sec-Fetch-Mode", "cors".parse().unwrap());
    headers.insert("Sec-Fetch-Site", "same-origin".parse().unwrap());


    match client.post(URL::AJAX).headers(headers).query(&[("f", "rsaPublicKey")]).send().await {
        Ok(response) => {

            #[derive(Debug, Deserialize)]
            #[serde(rename_all = "lowercase")]
            // From the hearth
            struct FuckYouLanis {
                publickey: String,
            }

            let response_json = response.text().await.unwrap();
            let json: FuckYouLanis = serde_json::from_str(&response_json).unwrap();
            let public_key = json.publickey;
            let public_key = RsaPublicKey::from_public_key_pem(&public_key).unwrap();

            Ok(public_key)
        }
        Err(e) => {
            Err(format!("Failed to get public key with error: {}", e))
        }
    }
}
