#![allow(dead_code)] //todo: remove after being tested and implemented

mod authentication;
mod certificate;
mod shared_key;

#[cfg(test)]
mod tests;

pub use authentication::*;
pub use certificate::*;
pub use shared_key::*;

pub const AUTH_CERT_EXPIRATION_LIMIT: u64 = 360000; // 60 minutes
