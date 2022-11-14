use runtime::types::H256;

pub struct StellarWallet {}

impl StellarWallet {
	pub(crate) async fn create_and_send_transaction(
		&self,
		p0: _,
		p1: u64,
		p2: _,
		p3: Option<H256>,
	) {
		todo!()
	}
}

impl StellarWallet {
	pub fn from_secret_encoded() -> Self {
		StellarWallet {}
	}
}
