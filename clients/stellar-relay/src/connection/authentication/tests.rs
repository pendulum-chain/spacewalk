use crate::{
	connection::{
		authentication::{
			certificate::{create_auth_cert, verify_remote_auth_cert},
			shared_key::gen_shared_key,
			ConnectionAuth, AUTH_CERT_EXPIRATION_LIMIT,
		},
		helper::{generate_random_nonce, time_now},
		hmac::{create_receiving_mac_key, create_sending_mac_key, create_sha256_hmac, verify_hmac},
	},
	Error,
};
use substrate_stellar_sdk::{
	network::Network,
	types::{AuthCert, Curve25519Public, HmacSha256Mac},
	SecretKey, XdrCodec,
};

fn mock_connection_auth() -> ConnectionAuth {
	let public_network = Network::new(b"Public Global Stellar Network ; September 2015");
	let secret =
		SecretKey::from_encoding("SCV6Q3VU4S52KVNJOFXWTHFUPHUKVYK3UV2ISGRLIUH54UGC6OPZVK2D")
			.expect("should work");

	ConnectionAuth::new(public_network.get_id(), secret, 0)
}

#[test]
fn create_valid_auth_cert() {
	let mut auth = mock_connection_auth();
	let time_now = time_now();

	let auth_cert =
		create_auth_cert(auth.network_id(), auth.keypair(), time_now, auth.pub_key_ecdh().clone())
			.expect("should successfully create auth cert");

	auth.set_auth_cert(auth_cert.clone());

	let mut network_id_xdr = auth.network_id().to_xdr();
	let pub_key = auth.keypair().get_public();
	assert!(verify_remote_auth_cert(time_now, pub_key, &auth_cert, &mut network_id_xdr));
}

#[test]
fn expired_auth_cert() {
	let mut auth = mock_connection_auth();

	let time_now = time_now();

	if let Err(Error::AuthCertNotFound) = auth.auth_cert(time_now) {
		assert!(true);
	} else {
		assert!(false);
	}

	let new_auth_cert =
		create_auth_cert(auth.network_id(), auth.keypair(), time_now, auth.pub_key_ecdh().clone())
			.expect("should successfully create an auth cert");

	auth.set_auth_cert(new_auth_cert.clone());

	let auth_inside_valid_range = auth.auth_cert(time_now + 1).expect("should return an auth cert");
	assert_eq!(&new_auth_cert, auth_inside_valid_range);

	// expired
	let new_time = time_now + (AUTH_CERT_EXPIRATION_LIMIT / 2) + 100;

	if let Err(Error::AuthCertExpired) = auth.auth_cert(new_time) {
		assert!(true);
	} else {
		assert!(false);
	}
}

#[test]
fn create_valid_shared_key() {
	let we_called_remote = true;
	let public_network = Network::new(b"Public Global Stellar Network ; September 2015");
	let secret =
		SecretKey::from_encoding("SCV6Q3VU4S52KVNJOFXWTHFUPHUKVYK3UV2ISGRLIUH54UGC6OPZVK2D")
			.expect("should work");

	let mut auth = ConnectionAuth::new(public_network.get_id(), secret, 0);

	let bytes =
		base64::decode_config("SaINZpCTl6KO8xMLvDkE2vE3knQz0Ma1RmJySOFqsWk=", base64::STANDARD)
			.expect("should be able to decode to bytes");

	let remote_pub_key = Curve25519Public {
		key: bytes.try_into().expect("should be able to convert to array of 32"),
	};

	assert!(auth.shared_key(&remote_pub_key, we_called_remote).is_none());

	let shared_key = gen_shared_key(
		&remote_pub_key,
		auth.secret_key_ecdh(),
		&auth.pub_key_ecdh(),
		we_called_remote,
	);

	auth.set_shared_key(&remote_pub_key, shared_key.clone(), we_called_remote);

	assert_eq!(auth.shared_key(&remote_pub_key, true), Some(&shared_key));
}

#[test]
fn mac_test() {
	fn data_message() -> Vec<u8> {
		let mut peer_sequence = 10u64.to_xdr();

		let mut message = base64::decode_config(
            "AAAAAAAAAAAAAAE3AAAACwAAAACslTOENMyaVlaiRvFAjiP6s8nFVIHDgWGbncnw+ziO5gAAAAACKbcUAAAAAzQaCq4p6tLHpdfwGhnlyX9dMUP70r4Dm98Td6YvKnhoAAAAAQAAAJijLxoAW1ZSaVphczIXU0XT7i46Jla6OZxkm9mEUfan3gAAAABg6Ee9AAAAAAAAAAEAAAAA+wsSteGzmcH88GN69FRjGLfxMzFH8tsJTaK+8ERePJMAAABAOiGtC3MiMa3LVn8f6SwUpKOmSMAJWQt2vewgt8T9WkRUPt2UdYac7vzcisXnmiusHldZcjVMF3vS03QhzaxdDQAAAAEAAACYoy8aAFtWUmlaYXMyF1NF0+4uOiZWujmcZJvZhFH2p94AAAAAYOhHvQAAAAAAAAABAAAAAPsLErXhs5nB/PBjevRUYxi38TMxR/LbCU2ivvBEXjyTAAAAQDohrQtzIjGty1Z/H+ksFKSjpkjACVkLdr3sILfE/VpEVD7dlHWGnO783IrF55orrB5XWXI1TBd70tN0Ic2sXQ0AAABA0ZiyH9AGgPR/d3h+94s6+iU5zhZbKM/5DIOYeKgxwEOotUveGfHLN5IQk7VlTW2arDkk+ekzjRQfBoexrkJrBMsQ30YpI1R/uY9npg0Fpt1ScyZ+yhABs6x1sEGminNh",
            base64::STANDARD,
        ) .expect("should be able to decode to bytes");

		let mut buf = vec![];
		buf.append(&mut peer_sequence);
		buf.append(&mut message);

		buf
	}

	let mut con_auth = mock_connection_auth();

	let public_network = Network::new(b"Public Global Stellar Network ; September 2015");
	let secret =
		SecretKey::from_encoding("SDAL6QYZG7O26OTLLP7JLNSB6SHY3CBZGJAWDPHYMRW2J3D2SA2RWU3L")
			.expect("should work");
	let mut peer_auth = ConnectionAuth::new(public_network.get_id(), secret, 0);

	let our_nonce = generate_random_nonce();
	let peer_nonce = generate_random_nonce();

	let recv_mac_key = {
		let remote_pub_key = Curve25519Public { key: peer_auth.pub_key_ecdh().key };

		let shared_key = gen_shared_key(
			&remote_pub_key,
			con_auth.secret_key_ecdh(),
			con_auth.pub_key_ecdh(),
			true,
		);

		con_auth.set_shared_key(&remote_pub_key, shared_key.clone(), true);

		create_receiving_mac_key(&shared_key, our_nonce, peer_nonce, true)
	};

	let peer_sending_mac_key = {
		let remote_pub_key = Curve25519Public { key: con_auth.pub_key_ecdh().key };

		let shared_key = gen_shared_key(
			&remote_pub_key,
			peer_auth.secret_key_ecdh(),
			peer_auth.pub_key_ecdh(),
			false,
		);

		peer_auth.set_shared_key(&remote_pub_key, shared_key.clone(), false);

		create_sending_mac_key(&shared_key, peer_nonce, our_nonce, false)
	};

	let mac_peer_uses_to_send_us_msg =
		create_sha256_hmac(&data_message(), &peer_sending_mac_key.mac)
			.unwrap_or(HmacSha256Mac { mac: [0; 32] });

	assert!(verify_hmac(
		&data_message(),                   // 3
		&recv_mac_key.mac,                 // 2
		&mac_peer_uses_to_send_us_msg.mac  // 1
	)
	.is_ok());
}
