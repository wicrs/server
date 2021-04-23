use std::convert::TryFrom;

use crate::error::Result;
use crate::{channel::Message, error::Error};
use pgp::crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm};
use pgp::packet::LiteralData;
use pgp::types::KeyTrait;
use pgp::types::{CompressionAlgorithm, SecretKeyTrait};
use pgp::Deserializable;
use pgp::{
    composed::{
        key::SecretKeyParamsBuilder, KeyType, Message as OpenPGPMessage, SignedPublicKey,
        SignedSecretKey,
    },
    types::PublicKeyTrait,
};
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

impl Message {
    pub fn sign<F: FnOnce() -> String>(
        &self,
        secret_key: &impl SecretKeyTrait,
        password: F,
    ) -> Result<OpenPGPMessage> {
        Ok(OpenPGPMessage::try_from(self)?.sign(&secret_key, password, HashAlgorithm::SHA2_256)?)
    }
}

impl TryFrom<&Message> for OpenPGPMessage {
    type Error = serde_json::Error;

    fn try_from(m: &Message) -> Result<Self, Self::Error> {
        Ok(OpenPGPMessage::Literal(LiteralData::from_str(
            &m.id.to_string(),
            &serde_json::to_string(&m)?,
        )))
    }
}

impl TryFrom<&OpenPGPMessage> for Message {
    type Error = Error;

    fn try_from(value: &OpenPGPMessage) -> Result<Self, Self::Error> {
        Ok(serde_json::from_str(
            &value
                .get_literal()
                .map_or_else(|| Err(Error::InvalidMessage), |s| Ok(s))?
                .to_string()
                .map_or_else(|| Err(Error::InvalidMessage), |s| Ok(s))?,
        )?)
    }
}

pub fn sign_final<F: FnOnce() -> String>(
    message_str: &str,
    server_public_key: &impl PublicKeyTrait,
    client_secret_key: &impl SecretKeyTrait,
    password: F,
) -> Result<OpenPGPMessage> {
    let pgp_message = OpenPGPMessage::from_string(message_str)?.0;
    pgp_message.verify(server_public_key)?;
    let message = Message::try_from(&pgp_message)?;
    Ok(
        OpenPGPMessage::Literal(LiteralData::from_str(&message.id.to_string(), message_str)).sign(
            client_secret_key,
            password,
            HashAlgorithm::SHA2_256,
        )?,
    )
}

pub fn sign_and_verify() -> Result {
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

    let passwd_fn = || String::new();

    let msg_signed = message.sign(&secret_key, passwd_fn)?;
    let msg_armored_str = msg_signed.to_armored_string(None)?;
    let msg_signature = msg_signed.clone().into_signature().signature;
    println!("{}", msg_armored_str);
    println!(
        "issuer: {}",
        hex::encode(msg_signature.issuer().unwrap().to_vec())
    );

    let _ = println!(
        "{}",
        Message::try_from(&dbg!(OpenPGPMessage::from_string(&msg_armored_str)?.0))?
    );

    let _ = sign_final(&msg_armored_str, &public_key, &secret_key, passwd_fn)?;
    Ok(())
}
