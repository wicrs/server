use std::{collections::HashMap, fmt, str};

use crate::new_id;
use crate::{error::Error, ID};
use pgp::{
    packet::LiteralData,
    types::{KeyTrait, SecretKeyTrait},
    Deserializable, Message, SignedPublicKey, SignedSecretKey, StandaloneSignature,
};

use serde::{
    de::{MapAccess, SeqAccess, Unexpected, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::error::Result;

pub const ACCOUNT_DATA_FOLDER: &str = "data/accounts/";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub public_keys: HashMap<String, String>,
    pub primary_fingerprint: String,
    pub uuid: ID,
}

impl Account {
    pub fn new(public_key: &SignedPublicKey, key_fingerprint: String) -> Result<Self> {
        let mut public_keys = HashMap::new();
        let _ = public_keys.insert(key_fingerprint.clone(), public_key.to_armored_string(None)?);
        Ok(Self {
            public_keys,
            uuid: new_id(),
            primary_fingerprint: key_fingerprint,
        })
    }

    pub fn sign<F>(self, secret_key: &SignedSecretKey, key_pw: F) -> Result<SignedAccount>
    where
        F: FnOnce() -> String,
    {
        let account_data = LiteralData::from_str("", &self.to_string());
        let signature = Message::Literal(account_data)
            .sign(secret_key, key_pw, pgp::crypto::HashAlgorithm::SHA2_256)?
            .into_signature();
        Ok(SignedAccount {
            account: self,
            signature,
        })
    }
}

impl ToString for Account {
    fn to_string(&self) -> String {
        let mut pubkey_string = String::new();
        for (fingerprint, public_key) in self.public_keys.keys().zip(self.public_keys.values()) {
            pubkey_string = format!("{},{}:{}", pubkey_string, fingerprint, public_key);
        }
        pubkey_string.remove(0);
        format!(
            "{} {} {}",
            pubkey_string,
            self.primary_fingerprint,
            self.uuid.to_string()
        )
    }
}

#[derive(Debug, Clone)]
pub struct SignedAccount {
    pub account: Account,
    pub signature: StandaloneSignature,
}

impl SignedAccount {
    pub fn new<F>(secret_key: &SignedSecretKey, key_pw: F) -> Result<Self>
    where
        F: FnOnce() -> String + Clone,
    {
        let fingerprint = hex::encode_upper(secret_key.fingerprint());
        let public_key = secret_key.public_key().sign(&secret_key, key_pw.clone())?;
        let account = Account::new(&public_key, fingerprint)?;
        account.sign(secret_key, key_pw)
    }

    /// Saves the account's data to disk.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The account data could not be serialized.
    /// * The account data folder does not exist and could not be created.
    /// * The data could not be written to the disk.
    pub async fn save(&self) -> Result {
        tokio::fs::create_dir_all(ACCOUNT_DATA_FOLDER).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&format!(
                "{}{:x}",
                ACCOUNT_DATA_FOLDER,
                self.account.uuid.as_u128()
            ))
            .await?;
        let bytes = bincode::serialize(self)?;
        let mut buf: &[u8] = bytes.as_slice();
        file.write_buf(&mut buf).await?;
        file.flush().await?;
        Ok(())
    }

    /// Loads an account's data given its ID.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * There is no account with that ID.
    /// * The account's data file was corrupt and could not be deserialized.
    pub async fn load(id: ID) -> Result<Self> {
        let filename = format!("{}{:x}", ACCOUNT_DATA_FOLDER, id.as_u128());
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(Error::AccountNotFound);
        }
        let mut file = tokio::fs::OpenOptions::new().read(true).open(path).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        Ok(bincode::deserialize(&buf)?)
    }
}

impl Serialize for SignedAccount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SignedAccount", 2)?;
        state.serialize_field("account", &self.account)?;
        let sig_string = self.signature.to_armored_string(None).map_err(|_| {
            serde::ser::Error::custom("failed to turn the signature into armoured bytes")
        })?;
        state.serialize_field("signature", &sig_string)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SignedAccount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Account,
            Signature,
        }

        struct SignedAccountVisitor;

        impl<'de> Visitor<'de> for SignedAccountVisitor {
            type Value = SignedAccount;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SignedAccount")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<SignedAccount, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let account = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let armoured_signature: String = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let signature = Message::from_string(&armoured_signature)
                    .map_err(|_| {
                        serde::de::Error::invalid_value(
                            Unexpected::Str("not a valid pgp armoured signature"),
                            &"a valid pgp armoured signature",
                        )
                    })?
                    .0
                    .into_signature();

                Ok(SignedAccount { account, signature })
            }

            fn visit_map<V>(self, mut map: V) -> Result<SignedAccount, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut account = None;
                let mut signature = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Account => {
                            if account.is_some() {
                                return Err(serde::de::Error::duplicate_field("account"));
                            }
                            account = Some(map.next_value()?);
                        }
                        Field::Signature => {
                            if signature.is_some() {
                                return Err(serde::de::Error::duplicate_field("signature"));
                            }
                            let armoured_signature: String = map.next_value()?;
                            signature = Some(
                                Message::from_string(&armoured_signature)
                                    .map_err(|_| {
                                        serde::de::Error::invalid_value(
                                            Unexpected::Str("not a valid pgp armoured signature"),
                                            &"a valid pgp armoured signature",
                                        )
                                    })?
                                    .0
                                    .into_signature(),
                            );
                        }
                    }
                }
                let account = account.ok_or_else(|| serde::de::Error::missing_field("secs"))?;
                let signature =
                    signature.ok_or_else(|| serde::de::Error::missing_field("nanos"))?;
                Ok(SignedAccount { account, signature })
            }
        }

        const FIELDS: &'static [&'static str] = &["account", "signature"];
        deserializer.deserialize_struct("SignedAccount", FIELDS, SignedAccountVisitor)
    }
}
