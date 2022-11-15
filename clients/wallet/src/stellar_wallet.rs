pub type StellarPublicKey = [u8; 32];

#[derive(Clone, PartialEq, Debug, Eq)]
pub struct StellarWallet {}

impl StellarWallet {
	pub fn get_public_key(&self) -> StellarPublicKey {
		// todo!()
		[0; 32]
	}
}

impl StellarWallet {
	pub fn from_secret_encoded(x: &String) -> Self {
		StellarWallet {}
	}
}
