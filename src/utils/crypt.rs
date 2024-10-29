use reqwest::Client;
use reqwest::header::HeaderMap;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, DecodePublicKey};
use serde::Deserialize;
use md5::Md5;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::cipher::block_padding::{NoPadding, Pkcs7};
use evpkdf::evpkdf;
use rand::random;
use regex::Regex;
use crate::utils::constants::URL;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub private_key: RsaPrivateKey,
    pub public_key: RsaPublicKey,
    /// Private key in PKCS#8 format
    pub private_key_string: String,
    /// Public key in PKCS#8 format
    pub public_key_string: String,
    /// Public key that's encoded and encrypted
    pub public_key_lanis: String,
}

/// Takes key_size (in bits) and returns an RSA KeyPair
pub async fn generate_key_pair(key_size: usize, client: &Client) -> Result<KeyPair, String> {
    let mut rng = rand::thread_rng();
    match RsaPrivateKey::new(&mut rng, key_size) {
        Ok(private_key) => {
            let public_key = RsaPublicKey::from(&private_key);

            let private_key_string = private_key.to_pkcs8_pem(Default::default());
            let public_key_string = public_key.to_public_key_pem(Default::default());

            if private_key_string.is_ok() && public_key_string.is_ok() {
                let private_key_string = private_key_string.unwrap().to_string();
                let public_key_string = public_key_string.unwrap();

                match handshake(client, &public_key_string).await {
                    Ok(public_key_lanis) => {
                        Ok(KeyPair{
                            private_key,
                            public_key,
                            private_key_string,
                            public_key_string,
                            public_key_lanis,
                        })
                    }
                    Err(e) => Err(format!("Handshake with lanis failed with error: '{}'", e)),
                }

            } else {
                Err("Failed to convert private key and/or public key to pkcs8 pem!".to_string())
            }
        }
        Err(e) => {
            Err(format!("Failed to generate Private key!: {}", e))
        }
    }
}

async fn handshake(client: &Client, public_own_key: &String) -> Result<String, String> {
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

            match client.post(URL::AJAX).headers(headers).query(&[("f", "rsaHandshake"), ("s", "1111")]).form(&[("key", &encrypted_key)]).send().await {
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
                                    let result_string = String::from_utf8_lossy(&result);
                                    let result_string = result_string.trim();
                                    if result_string == public_own_key.trim() {
                                        Ok(encrypted_key)
                                    } else {
                                        Err(format!("Failed to perform challenge! Public Keys don't match!:\nOwn Public Key:\n{}\n\nResponse Public Key:\n{}", public_own_key, result_string))
                                    }
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

async fn get_public_key(client: &Client) -> Result<RsaPublicKey, String> {
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

pub (crate) async fn encrypt_data(data: &[u8], public_key: &String) -> Result<String, String> {
    let salt = random::<[u8; 8]>();

    const KEY_SIZE: usize = 256;
    const IV_SIZE: usize = 128;

    let mut output = [0; (KEY_SIZE + IV_SIZE) / 8];
    evpkdf::<Md5>(public_key.as_bytes(), &salt, 1, &mut output);

    let (key, iv) = output.split_at(KEY_SIZE / 8);

    let key: [u8; 32] = key.try_into().unwrap();
    let iv: [u8; 16] = iv.try_into().unwrap();

    let encryptor = Aes256CbcEnc::new(&key.into(), &iv.into());

    let encrypted = {

        let salted = "Salted__".to_string();
        let salted = salted.as_bytes();

        let mut buf = [0; 256];
        let encrypted = encryptor.encrypt_padded_b2b_mut::<Pkcs7>(&data, &mut buf);
        if encrypted.is_err() {
            return Err(format!("Failed to encrypt data with error: '{}'", encrypted.unwrap_err()));
        }
        let encrypted = encrypted.unwrap();

        let mut result: Vec<u8> = Vec::new();
        result.extend(salted);
        result.extend(salt);
        result.extend(encrypted);

        result
    };

    let result = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, encrypted);

    Ok(result)
}


pub (crate) async fn decrypt_encoded_tags(html_string: &str, key: &String) -> Result<String, String> {
    let exp = Regex::new(r"<encoded>(.*?)</encoded>").unwrap();

    let mut replaced_html = html_string.to_string();

    for caps in exp.captures_iter(html_string) {
        if let Some(encoded_content) = caps.get(1) {
            let decrypted_content = decrypt_string_with_key(encoded_content.as_str(), key).await;
            let decrypted_string = decrypted_content.unwrap_or_default();
            replaced_html = replaced_html.replacen(&caps[0], &decrypted_string, 1);
        }
    }

    Ok(replaced_html.to_string())
}

pub(crate) async fn decrypt_string_with_key(data: &str, public_key: &String) -> Result<String, String> {
    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data) {
        Ok(data) => {
            let result = decrypt_with_key(&data, &public_key).await?;
            let result_string = String::from_utf8_lossy(&result);
            let result_string = result_string.trim();
            Ok(result_string.to_string())
        }
        Err(e) => {
            Err(format!("Failed to decode base64 string with error: '{}'", e))
        }
    }
}

pub(crate) async fn decrypt_with_key(data: &Vec<u8>, public_key: &String) -> Result<Vec<u8>, String> {
    fn is_salted(encrypted_data: &Vec<u8>) -> bool {
        match std::str::from_utf8(&encrypted_data[0..8]) {
            Ok(s) => s == "Salted__",
            Err(_) => false,
        }
    }

    if !is_salted(&data) {
        return Err("Data is not salted!".to_string());
    }

    let salt = &data[8..16];

    const KEY_SIZE: usize = 256;
    const IV_SIZE: usize = 128;

    let mut output = [0; (KEY_SIZE + IV_SIZE) / 8];

    evpkdf::<Md5>(public_key.as_bytes(), salt, 1, &mut output);

    let (key, iv) = output.split_at(KEY_SIZE / 8);

    let key: [u8; 32] = key.try_into().unwrap();
    let iv: [u8; 16] = iv.try_into().unwrap();


    let decryptor = Aes256CbcDec::new(&key.into(), &iv.into());

    let result = decryptor.decrypt_padded_vec_mut::<NoPadding>(&data[16..]).unwrap();

    Ok(result)

}
