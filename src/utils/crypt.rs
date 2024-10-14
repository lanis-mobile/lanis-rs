use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};

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
pub fn generate_key_pair(key_size: usize) -> Result<KeyPair, String> {
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

