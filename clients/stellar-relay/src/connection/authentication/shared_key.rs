use crate::connection::{authentication::ConnectionAuth, hmac::create_sha256_hmac};
use substrate_stellar_sdk::{
    types::{Curve25519Public, HmacSha256Mac},
    Curve25519Secret,
};

impl ConnectionAuth {
    /// Gets an existing shared key.
    /// Returns `none` when not found.
    pub fn shared_key(&self, remote_pub_key_ecdh: &Curve25519Public, we_called_remote: bool) -> Option<&HmacSha256Mac> {
        let shared_keys_map = if we_called_remote {
            self.we_called_remote_shared_keys()
        } else {
            self.remote_called_us_shared_keys()
        };

        shared_keys_map.get(&remote_pub_key_ecdh.key)
    }

    pub fn set_shared_key(
        &mut self,
        remote_pub_key_ecdh: &Curve25519Public,
        shared_key: HmacSha256Mac,
        we_called_remote: bool,
    ) {
        // save the hmac
        if we_called_remote {
            self.insert_we_called_remote_shared_keys(remote_pub_key_ecdh.key, shared_key);
        } else {
            self.insert_remote_called_us_shared_keys(remote_pub_key_ecdh.key, shared_key);
        };
    }
}

pub fn gen_shared_key(
    remote_pub_key_ecdh: &Curve25519Public,
    secret_key_ecdh: &Curve25519Secret,
    pub_key_ecdh: &Curve25519Public,
    we_called_remote: bool,
) -> HmacSha256Mac {
    // prepare the buffers
    let mut final_buffer: Vec<u8> = vec![];
    let mut buffer: [u8; 32] = [0; 32];

    tweetnacl::scalarmult(&mut buffer, &secret_key_ecdh.key, &remote_pub_key_ecdh.key);

    final_buffer.extend_from_slice(&buffer);
    if we_called_remote {
        final_buffer.extend_from_slice(&pub_key_ecdh.key);
        final_buffer.extend_from_slice(&remote_pub_key_ecdh.key);
    } else {
        final_buffer.extend_from_slice(&remote_pub_key_ecdh.key);
        final_buffer.extend_from_slice(&pub_key_ecdh.key);
    }

    create_sha256_hmac(&final_buffer, &[0; 32]).unwrap_or(HmacSha256Mac { mac: [0; 32] })
}
