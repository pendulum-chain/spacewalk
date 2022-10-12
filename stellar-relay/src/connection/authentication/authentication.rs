use rand::Rng;
use std::collections::HashMap;
use substrate_stellar_sdk::types::{AuthCert, Curve25519Public, HmacSha256Mac};
use substrate_stellar_sdk::{Curve25519Secret, SecretKey};

type KeyAsBinary = [u8; 32];
pub type BinarySha256Hash = [u8; 32];

pub struct ConnectionAuth {
    keypair: SecretKey,
    secret_key_ecdh: Curve25519Secret,
    pub_key_ecdh: Curve25519Public,
    network_hash: BinarySha256Hash,
    we_called_remote_shared_keys: HashMap<KeyAsBinary, HmacSha256Mac>,
    remote_called_us_shared_keys: HashMap<KeyAsBinary, HmacSha256Mac>,
    auth_cert: Option<AuthCert>,
    auth_cert_expiration: u64,
}

impl ConnectionAuth {
    pub fn new(
        network: &BinarySha256Hash,
        keypair: SecretKey,
        auth_cert_expiration: u64,
    ) -> ConnectionAuth {
        let secret_key = rand::thread_rng().gen::<KeyAsBinary>();

        let mut pub_key: KeyAsBinary = [0; 32];
        tweetnacl::scalarmult_base(&mut pub_key, &secret_key);

        ConnectionAuth {
            keypair,
            secret_key_ecdh: Curve25519Secret { key: secret_key },
            pub_key_ecdh: Curve25519Public { key: pub_key },
            network_hash: *network,
            we_called_remote_shared_keys: HashMap::new(),
            remote_called_us_shared_keys: HashMap::new(),
            auth_cert: None,
            auth_cert_expiration,
        }
    }

    pub fn keypair(&self) -> &SecretKey {
        &self.keypair
    }

    pub fn secret_key_ecdh(&self) -> &Curve25519Secret {
        &self.secret_key_ecdh
    }

    pub fn pub_key_ecdh(&self) -> &Curve25519Public {
        &self.pub_key_ecdh
    }

    pub fn network_id(&self) -> &BinarySha256Hash {
        &self.network_hash
    }

    pub fn we_called_remote_shared_keys(&self) -> &HashMap<KeyAsBinary, HmacSha256Mac> {
        &self.we_called_remote_shared_keys
    }

    pub fn remote_called_us_shared_keys(&self) -> &HashMap<KeyAsBinary, HmacSha256Mac> {
        &self.remote_called_us_shared_keys
    }

    pub(super) fn saved_auth_cert(&self) -> Option<&AuthCert> {
        self.auth_cert.as_ref()
    }

    pub fn update_auth_cert(&mut self, cert: AuthCert) {
        self.auth_cert = Some(cert);
    }

    pub fn auth_cert_expiration(&self) -> u64 {
        self.auth_cert_expiration
    }

    pub fn update_auth_cert_expiration(&mut self, expiration: u64) {
        self.auth_cert_expiration = expiration;
    }

    pub fn insert_we_called_remote_shared_keys(&mut self, key: KeyAsBinary, value: HmacSha256Mac) {
        self.we_called_remote_shared_keys.insert(key, value);
    }

    pub fn insert_remote_called_us_shared_keys(&mut self, key: KeyAsBinary, value: HmacSha256Mac) {
        self.remote_called_us_shared_keys.insert(key, value);
    }
}
