use std::fmt::Write;
use std::{collections::HashMap, str};

use crate::new_id;
use crate::{error::Error, ID};
use chrono::{DateTime, Utc};
use pgp::{
    types::{KeyTrait, SecretKeyTrait},
    SignedSecretKey,
};
use pgp::{Deserializable, Message, Signature, SignedPublicKey};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::error::Result;

pub const ACCOUNT_DATA_FOLDER: &str = "data/accounts/";

/// Actions that can be added to accounts to indicate changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum AccountAction {
    Create(String, usize),
    AuthPublicKey(String, usize),
    DeAuthPublicKey(String, usize),
}

impl ToString for AccountAction {
    fn to_string(&self) -> String {
        match self {
            Self::Create(key_id, key_count) => format!("create({},{})", key_id, key_count),
            Self::AuthPublicKey(key_id, key_count) => {
                format!("authpublickey({},{})", key_id, key_count)
            }
            Self::DeAuthPublicKey(key_id, key_count) => {
                format!("deauthpublickey({},{})", key_id, key_count)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub public_keys: HashMap<String, (String, DateTime<Utc>)>,
    pub uuid: ID,
    pub actions: Vec<AccountAction>,
    pub action_signatures: Vec<String>,
}

impl Account {
    /// Creates a new account and signs it.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The password given for the secret key is wrong.
    /// * The account could not be signed.
    /// * The public key could not be signed or could not be armored.
    pub fn new<F>(secret_key: &SignedSecretKey, key_pw: F) -> Result<Self>
    where
        F: FnOnce() -> String + Clone,
    {
        let key_id = hex::encode_upper(secret_key.key_id().to_vec());
        let public_key = secret_key.public_key().sign(&secret_key, key_pw.clone())?;
        let mut pubkey_map = HashMap::new();
        pubkey_map.insert(
            key_id.clone(),
            (public_key.to_armored_string(None)?, Utc::now()),
        );
        let account = Self {
            uuid: new_id(),
            public_keys: pubkey_map,
            actions: vec![AccountAction::Create(key_id, 1)],
            action_signatures: vec![],
        };
        account.sign_last_action(secret_key, key_pw)
    }

    /// Signs the most recent action, panics if the most recent action has already been signed.
    pub fn sign_last_action<F>(mut self, secret_key: &SignedSecretKey, key_pw: F) -> Result<Self> {
        if self.actions.len() < 1 || self.actions.len() - 1 != self.action_signatures.len() {
            Err(Error::NoActionToSign)
        } else {
            Ok(self)
        }
    }

    /// Verifies that an account is correctly signed.
    pub fn verify(&self) -> Result {
        if self.actions.len() != self.action_signatures.len() {
            return Err(Error::AccountNotBalanced);
        }
        if self.action_signatures.len() < 1 {
            return Err(Error::AccountNotSigned);
        }
        let mut subject = self.clone();
        while subject.actions.len() > 0 && subject.action_signatures.len() > 0 {
            subject.verify_last_action()?;
            subject.actions.pop();
        }
        Ok(())
    }

    /// Checks that the last action on an account is signed and that the signer is authorized to sign the account.
    fn verify_last_action(&mut self) -> Result {
        if self.actions.len() != self.action_signatures.len() {
            return Err(Error::AccountNotBalanced);
        }
        if self.action_signatures.len() < 1 {
            return Err(Error::AccountNotSigned);
        }
        let (key_id, _) = Self::get_action_issuer(&self.action_signatures.pop().unwrap())?;
        Ok(())
    }

    fn get_action_issuer(signature: &String) -> Result<(String, Signature)> {
        let message = Message::from_string(signature)?.0;
        let message = message.decompress()?;
        if let pgp::composed::message::Message::Signed {
            message: _,
            one_pass_signature: _,
            signature,
        } = message
        {
            if let Some(issuer_key_id) = signature.issuer() {
                Ok((hex::encode_upper(issuer_key_id.to_vec()), signature))
            } else {
                Err(Error::InvalidMessage)
            }
        } else {
            Err(Error::InvalidMessage)
        }
    }

    pub fn only_first_n_actions(
        self,
        action_count: usize,
        sorted_public_keys: &Vec<(&String, &(String, DateTime<Utc>))>,
    ) -> Self {
        let mut public_keys = HashMap::new();
        sorted_public_keys
            .iter()
            .take(action_count)
            .cloned()
            .for_each(|(id, key_with_added_time)| {
                public_keys
                    .insert(id.clone(), key_with_added_time.clone())
                    .expect("failed to create temporary key map");
            });
        Self {
            public_keys,
            uuid: self.uuid.clone(),
            actions: self.actions.iter().take(action_count).cloned().collect(),
            action_signatures: self
                .action_signatures
                .iter()
                .take(action_count)
                .cloned()
                .collect(),
        }
    }

    pub fn public_keys_sorted(&self) -> Vec<(&String, &(String, DateTime<Utc>))> {
        let mut pubkeys = self
            .public_keys
            .iter()
            .collect::<Vec<(&String, &(String, DateTime<Utc>))>>();
        pubkeys.sort_by_key(|key_data| key_data.1 .1);
        pubkeys
    }

    /// Checks if a key is authorized to perform actions on an account.
    pub fn is_key_authorized(
        &self,
        key_id: String,
        sorted_public_keys: &Vec<(&String, &(String, DateTime<Utc>))>,
    ) -> Result<bool> {
        let mut authorized = false;
        let mut actions_iter = self.actions.iter().enumerate();
        while let Some((position, action)) = actions_iter.next() {
            match &action {
                AccountAction::AuthPublicKey(authed_key_id, pub_key_count) => {
                    let signature = self
                        .action_signatures
                        .get(position)
                        .expect("missing signature for action");
                    let (issuer, signature) = Self::get_action_issuer(signature)?;
                    if let Some((issuer_pubkey, _pubkey_added)) = self.public_keys.get(&issuer) {
                        signature.verify(
                            &SignedPublicKey::from_string(issuer_pubkey)?.0,
                            self.clone()
                                .only_first_n_actions(position + 1, &sorted_public_keys)
                                .to_string()
                                .as_bytes(),
                        )?;
                        authorized = true;
                    }
                }
                AccountAction::Create(authed_key_id, pub_key_count) => todo!(),
                AccountAction::DeAuthPublicKey(_, _) => todo!(),
            }
        }
        Ok(authorized)
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
            .open(&format!("{}{:x}", ACCOUNT_DATA_FOLDER, self.uuid.as_u128()))
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

impl ToString for Account {
    fn to_string(&self) -> String {
        let mut string = format!("{} ", self.uuid.to_string());
        let mut pubkeys = self
            .public_keys
            .clone()
            .into_iter()
            .collect::<Vec<(String, (String, DateTime<Utc>))>>();
        pubkeys.sort_by_key(|key_data| key_data.1 .1);
        for (fingerprint, (key, time)) in pubkeys {
            string
                .write_fmt(format_args!(
                    "{},({},{}) ",
                    fingerprint,
                    key,
                    time.to_string()
                ))
                .expect("failed to write public key data to string");
        }
        for action in &self.actions {
            string
                .write_fmt(format_args!("{} ", action.to_string()))
                .expect("failed to write account action to string");
        }
        for signature in &self.action_signatures {
            string
                .write_fmt(format_args!("{} ", signature))
                .expect("failed to write account action signature to string");
        }
        string
    }
}
