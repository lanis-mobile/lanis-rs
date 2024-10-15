use reqwest::Client;
use reqwest::header::HeaderMap;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, DecodePublicKey};
use serde::Deserialize;
use md5::{Digest, Md5};
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use aes::cipher::block_padding::NoPadding;
use crate::utils::constants::URL;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

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

pub(crate) async fn handshake(client: &Client, public_own_key: &String) -> Result<(), String> {
    let mut rng = rand::thread_rng();
    let public_key = get_public_key(&client).await?;

    match public_key.encrypt(&mut rng, Pkcs1v15Encrypt, public_own_key.as_bytes()) {
        Ok(encrypted_key) => {

            let encrypted_key = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, encrypted_key);

            let mut headers = HeaderMap::new();
            headers.insert("Accept", "*/*".parse().unwrap());
            headers.insert("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8".parse().unwrap());
            headers.insert("Sec-Fetch-Dest", "empty".parse().unwrap());
            headers.insert("Sec-Fetch-Mode", "cors".parse().unwrap());
            headers.insert("Sec-Fetch-Site", "same-origin".parse().unwrap());

            match client.post(URL::AJAX).headers(headers).query(&[("f", "rsaHandshake"), ("s", "1111")]).form(&[("key", encrypted_key)]).send().await {
                Ok(response) => {
                    #[derive(Debug, Deserialize)]
                    struct ResponseData {
                        challenge: String,
                    }

                    match serde_json::from_str::<ResponseData>(response.text().await.unwrap().as_str()) {
                        Ok(data) => {
                            match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data.challenge) {
                                Ok(challenge) => {
                                    let result = decrypt_with_key(&challenge, public_own_key).await?;
                                    Ok(())
                                }
                                Err(e) => {
                                    Err(format!("Failed to decode challenge with error: '{}'", e))
                                }
                            }
                        }
                        Err(e) => {
                            Err(format!("Failed to decode json with error: '{}'", e))
                        }
                    }
                }
                Err(e) =>  Err(format!("Failed to perform handshake with error: '{}'", e)),
            }
        }
        Err(e) => {
            Err(format!("Failed to encrypt with error: '{}'\nIs your public key to long? Maybe take a look at the documentation of the key 'key_pair' in struct 'Account'", e))
        }
    }
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

pub(crate) async fn decrypt_with_key(data: &Vec<u8>, public_key: &String) -> Result<Vec<u8>, String> {
    println!("Encrypted Data: \t\t\t'{}'", String::from_utf8_lossy(&data));


    fn is_salted(encrypted_data: &Vec<u8>) -> bool {
        match std::str::from_utf8(&encrypted_data[0..8]) {
            Ok(s) => s == "Salted__",
            Err(_) => false,
        }
    }

    pub fn bytes_to_keys(salt: &[u8], key: &[u8]) -> Vec<u8> {
        let mut concatenated_hashes = vec![];
        let mut current_hash = [0; 16];
        let mut enough_bytes_for_key = false;
        let mut pre_hash = vec![0; 0];

        loop {
            if !current_hash.iter().all(|&x| x == 0) {
                pre_hash.extend_from_slice(key);
                pre_hash.extend_from_slice(salt);
            } else {
                pre_hash.extend_from_slice(key);
                pre_hash.extend_from_slice(salt);
            }

            current_hash = [0; 16];
            let mut md5 = Md5::new();
            md5::Digest::update(&mut md5, &pre_hash);

            let mut hash_bytes: Vec<u8> = vec![];
            {
                let mut md5_clone = md5.clone();
                md5::Digest::update(&mut md5_clone, &[]);
                let result = md5_clone.finalize();
                hash_bytes.extend_from_slice(result.as_ref());
            }

            concatenated_hashes.extend_from_slice(&hash_bytes);

            if concatenated_hashes.len() >= 48 {
                enough_bytes_for_key = true;
                break;
            }
        }

        concatenated_hashes
    }

    if !is_salted(&data) {
        return Err("Data is not salted!".to_string());
    }

    let salt = &data[8..16];

    let key = bytes_to_keys(&salt, &public_key.as_bytes());

    let mut key_nv = [0; 32];
    let mut iv = [0; 16];

    for i in 0..32 {
        key_nv[i] = key[i]
    }

    for i in 0..16 {
        iv[i] = key[i+32]
    }

    let decryptor = Aes256CbcDec::new(&key_nv.into(), &iv.into());

    let plaintext = decryptor.decrypt_padded_vec_mut::<NoPadding>(&data[16..]).unwrap();

    println!("Decrypted Data: \t\t\t{}", String::from_utf8_lossy(&plaintext));

    Ok(vec![])

}
