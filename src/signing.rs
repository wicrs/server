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

pub const SECRET_KEY_PATH: &str = "data/secret_key.asc";
pub const PUBLIC_KEY_PATH: &str = "data/public_key.asc";
pub const USER_PUBLIC_KEY_FOLDER: &str = "data/user_public_keys/";

#[derive(Clone, Debug)]
pub struct KeyPair {
    pub secret_key: SignedSecretKey,
    pub public_key: SignedPublicKey,
}

impl KeyPair {
    pub fn new<S: Into<String>>(id: S) -> Result<Self> {
        let secret_key = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Rsa(4096))
            .can_create_certificates(true)
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

    pub async fn save(&self, secret_key_path: &str, public_key_path: &str) -> Result {
        tokio::fs::write(secret_key_path, self.secret_key.to_armored_bytes(None)?).await?;
        tokio::fs::write(public_key_path, self.public_key.to_armored_bytes(None)?).await?;
        Ok(())
    }

    pub async fn load(secret_key_path: &str, public_key_path: &str) -> Result<Self> {
        let secret_key =
            SignedSecretKey::from_string(&tokio::fs::read_to_string(secret_key_path).await?)?.0;
        let public_key =
            SignedPublicKey::from_string(&tokio::fs::read_to_string(public_key_path).await?)?.0;
        Ok(Self {
            secret_key,
            public_key,
        })
    }

    pub async fn load_or_create<S: Into<String>>(
        id: S,
        secret_key_path: &str,
        public_key_path: &str,
    ) -> Result<Self> {
        let result = if let Ok(key_pair) = KeyPair::load(secret_key_path, public_key_path).await {
            key_pair
        } else {
            let key_pair = KeyPair::new(id)?;
            key_pair.save(SECRET_KEY_PATH, PUBLIC_KEY_PATH).await?;
            key_pair
        };
        Ok(result)
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

    pub fn from_double_signed(message_str: &str) -> Result<Self> {
        let client_signed = OpenPGPMessage::from_string(message_str)?.0;
        if let Some(d) = client_signed.get_literal() {
            if let Some(s) = d.to_string() {
                return Message::try_from(OpenPGPMessage::from_string(&s)?.0);
            }
        }
        Err(Error::InvalidMessage)
    }

    pub fn from_double_signed_verify(
        message_str: &str,
        server_public_key: &impl PublicKeyTrait,
        client_public_key: &impl PublicKeyTrait,
    ) -> Result<Message> {
        let client_signed = OpenPGPMessage::from_string(message_str)?.0;
        client_signed.verify(client_public_key)?;
        if let Some(d) = client_signed.get_literal() {
            if let Some(s) = d.to_string() {
                let server_signed = OpenPGPMessage::from_string(&s)?.0;
                server_signed.verify(server_public_key)?;
                return Message::try_from(server_signed);
            }
        }
        Err(Error::InvalidMessage)
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
            OpenPGPMessage::Literal(LiteralData::from_str(&message.id.to_string(), message_str))
                .sign(client_secret_key, password, HashAlgorithm::SHA2_256)?,
        )
    }
}

impl TryFrom<&Message> for OpenPGPMessage {
    type Error = Error;

    fn try_from(m: &Message) -> Result<Self, Self::Error> {
        Ok(OpenPGPMessage::Literal(LiteralData::from_str(
            &m.id.to_string(),
            &serde_json::to_string(&m)?,
        )))
    }
}

impl TryFrom<Message> for OpenPGPMessage {
    type Error = Error;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&OpenPGPMessage> for Message {
    type Error = Error;

    fn try_from(value: &OpenPGPMessage) -> Result<Self, Self::Error> {
        Ok(serde_json::from_str(
            &value
                .get_literal()
                .map_or_else(|| Err(Error::InvalidMessage), Ok)?
                .to_string()
                .map_or_else(|| Err(Error::InvalidMessage), Ok)?,
        )?)
    }
}

impl TryFrom<OpenPGPMessage> for Message {
    type Error = Error;

    fn try_from(value: OpenPGPMessage) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

// Unsafe, here for reference if for some reason this is needed (appears to work with `pgp = "0.7.1"`)
// This was only thought of because the signature field in [`StandaloneSignature`] was private in the latest version of rpgp (at the time "0.7.1").
//
// pub trait GetSignature {
//     fn get_signature(self) -> Signature;
// }

// impl GetSignature for StandaloneSignature {
//     fn get_signature(self) -> Signature {
//         struct StandaloneSignature {
//             signature: Signature,
//         }
//         let transmuted: StandaloneSignature = unsafe { std::mem::transmute(self) };
//         transmuted.signature
//     }
// }

pub async fn get_or_import_public_key(fingerprint: &str) -> Result<SignedPublicKey> {
    let file_name = format!("{}{}.asc", USER_PUBLIC_KEY_FOLDER, fingerprint);
    let path = std::path::Path::new(&file_name);
    if path.is_file() {
        Ok(SignedPublicKey::from_string(&tokio::fs::read_to_string(path).await?)?.0)
    } else {
        Err(Error::PublicKeyNotFound)
    }
}

pub async fn sign_and_verify() -> Result {
    let KeyPair {
        secret_key,
        public_key,
    } = KeyPair::load_or_create(
        "WICRS Server <server@wic.rs>",
        SECRET_KEY_PATH,
        PUBLIC_KEY_PATH,
    )
    .await?;

    let message = Message::new("test".into(), "this is a test message".into());

    let passwd_fn = || String::new();

    let msg_signed = message.sign(&secret_key, passwd_fn)?;
    let msg_armored_str = msg_signed.to_armored_string(None)?;

    println!("{}\n", msg_armored_str);

    let final_message = Message::sign_final(&msg_armored_str, &public_key, &secret_key, passwd_fn)?;
    let final_message_str = final_message.to_armored_string(None)?;

    println!("{}\n", final_message_str);

    println!(
        "public_key id: {:?}\npublic_key user_id: {}\n",
        public_key.key_id(),
        public_key.details.users.first().unwrap().id,
    );

    let _ = println!(
        "{}",
        Message::from_double_signed_verify(&final_message_str, &public_key, &public_key)?
    );

    Ok(())
}
