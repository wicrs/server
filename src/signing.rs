use crate::channel::Message;
use crate::error::Result;
use pgp::composed::{
    key::SecretKeyParamsBuilder, KeyType, Message as OpenPGPMessage, SignedPublicKey,
    SignedSecretKey,
};
use pgp::crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm};
use pgp::packet::LiteralData;
use pgp::types::KeyTrait;
use pgp::types::{CompressionAlgorithm, SecretKeyTrait};
use pgp::Deserializable;
use smallvec::*;

pub const SECRET_KEY_PATH: &str = "data/secret_key";

pub struct KeyPair {
    pub secret_key: SignedSecretKey,
    pub public_key: SignedPublicKey,
}

impl KeyPair {
    pub fn new<S: Into<String>>(id: S) -> Result<Self> {
        let secret_key = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Rsa(2048))
            .can_create_certificates(false)
            .can_sign(true)
            .primary_user_id(id.into())
            .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256,])
            .preferred_hash_algorithms(smallvec![HashAlgorithm::SHA2_256,])
            .preferred_compression_algorithms(smallvec![CompressionAlgorithm::ZLIB,])
            .build()?
            .generate()?;
        let passwd_fn = || String::new();
        let secret_key = secret_key.sign(passwd_fn)?;
        let public_key = secret_key.public_key().sign(&secret_key, passwd_fn)?;
        Ok(Self {
            secret_key,
            public_key,
        })
    }

    pub fn save_secret_key(&self) -> Result {
        std::fs::write(SECRET_KEY_PATH, self.secret_key.to_armored_bytes(None)?)?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let secret_key =
            SignedSecretKey::from_string(&std::fs::read_to_string(SECRET_KEY_PATH)?)?.0;
        let passwd_fn = || String::new();
        let public_key = secret_key.public_key().sign(&secret_key, passwd_fn)?;
        Ok(Self {
            secret_key,
            public_key,
        })
    }
}

//pub fn sign_message(message: Message, secret_key: &SignedSecretKey) -> Result<OpenPGPMessage> {}

pub fn sign_and_verify() {
    let KeyPair {
        secret_key,
        public_key,
    } = if let Ok(key_pair) = KeyPair::load() {
        key_pair
    } else {
        let key_pair =
            KeyPair::new("WICRS Server <server@wic.rs>").expect("Failed to create a new key pair.");
        let _ = key_pair.save_secret_key();
        key_pair
    };

    println!("key_id: {}\n", hex::encode(secret_key.key_id().to_vec()));

    let message = Message::new("test".into(), "this is a test message".into());
    let message_json = serde_json::to_string(&message).unwrap();

    let passwd_fn = || String::new();

    let msg_signed = OpenPGPMessage::Literal(LiteralData::from_str(
        &message.id.to_string(),
        &message_json,
    ))
    .sign(&secret_key, passwd_fn, HashAlgorithm::SHA2_256)
    .expect("Failed to sign message.");
    let msg_armored_str = msg_signed
        .to_armored_string(None)
        .expect("Failed to turn signature into string.");
    println!("{}", msg_armored_str);

    let _ = println!(
        "{}",
        serde_json::from_str::<'_, Message>(
            &OpenPGPMessage::from_string(&msg_armored_str)
                .unwrap()
                .0
                .get_literal()
                .unwrap()
                .to_string()
                .unwrap()
        )
        .unwrap()
    );

    msg_signed
        .verify(&public_key)
        .map(|_| println!("Verification successful."))
        .expect("Failed to verify the message.");
}
