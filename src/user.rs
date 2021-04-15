use std::io::Read;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{
    digest::{ExtendableOutput, Update},
    Digest, Sha3_256, Shake128,
};

use crate::{
    auth::Service,
    error::{DataError, Error},
    hub::Hub,
    Result, ID,
};

use async_graphql::SimpleObject;

/// Relative path to the folder where user data is stored.
pub const USER_FOLDER: &str = "data/users/";

/// Represents a user of WICRS.
#[derive(SimpleObject, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    /// ID of the user.
    pub id: ID,
    /// The user's status.
    pub status: String,
    /// The user's description.
    pub description: String,
    /// The user's name.
    pub username: String,
    /// The email address used by the user on their OAuth service.
    pub email: String,
    /// Time of creation of the user in milliseconds from Unix Epoch.
    pub created: DateTime<Utc>,
    /// The OAuth service the user used to sign up.
    pub service: Service,
    /// A list of the hubs that the user is a member of.
    pub in_hubs: Vec<ID>,
}

/// Represents the publicly available information on a user, (excludes their email address and the service they signed up with) also only includes the generic version of accounts.
/// Refer to [`User`] for the use of the fields.
#[derive(SimpleObject, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericUser {
    pub id: ID,
    pub status: String,
    pub description: String,
    pub username: String,
    pub created: DateTime<Utc>,
    /// Hashed versions of the IDs of the hubs that the user is in.
    pub hubs_hashed: Vec<String>,
}

impl User {
    /// Creates a new user and generates an ID by hashing the service used and the ID of the user according to that service.
    pub fn new(id: String, email: String, service: Service) -> Self {
        Self {
            id: get_id(&id, &service),
            username: String::new(),
            status: String::new(),
            description: String::new(),
            email,
            service,
            created: Utc::now(),
            in_hubs: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new_for_testing(id_num: u128) -> Self {
        User {
            id: ID::from_u128(id_num),
            username: format!("test_user_{}", id_num),
            email: "test@example.com".to_string(),
            in_hubs: Vec::new(),
            status: "Testing stuff.".to_string(),
            description: "A user for testing purposes.".to_string(),
            created: Utc::now(),
            service: Service::GitHub,
        }
    }

    /// Converts the standard user into a GenericUser, the hashed versions of the hub ID list also use the requester's ID to avoid hash lists or rainbow tables being used.
    pub fn to_generic(&self, requester_id: &ID) -> GenericUser {
        let mut hasher = Sha3_256::new();
        let mut hubs_hashed = Vec::new();
        for hub in self.in_hubs.clone() {
            sha3::digest::Update::update(&mut hasher, hub.to_string());
            sha3::digest::Update::update(&mut hasher, requester_id.to_string());
            hubs_hashed.push(format!("{:x}", hasher.finalize_reset()));
        }
        GenericUser {
            id: self.id.clone(),
            created: self.created.clone(),
            description: self.description.clone(),
            status: self.status.clone(),
            username: self.username.clone(),
            hubs_hashed,
        }
    }

    /// Adds the user to the hub with the given ID, giving them the default permissions and making sure that they are not banned.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * Thu hub does not exist.
    /// * The user is banned from the hub.
    /// * The user could not be added to the hub for any of the reasons outlined in [`Hub::user_join`].
    /// * The hub failed to load for any of the reasons outlined in [`Hub::load`].
    /// * The hub failed to save for any of the reasons outlined in [`Hub::save`].
    pub fn join_hub(&mut self, hub: &mut Hub) -> Result {
        if hub.bans.contains(&self.id) {
            Err(Error::Banned)
        } else {
            hub.user_join(&self)?;
            self.in_hubs.push(hub.id.clone());
            Ok(())
        }
    }

    /// Removes the user from the hub with the given ID.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user is not a member of the given hub.
    pub fn remove_hub(&mut self, hub_id: &ID) -> Result {
        if let Some(index) = self.in_hubs.iter().position(|id| id == hub_id) {
            self.in_hubs.remove(index);
            Ok(())
        } else {
            Err(Error::NotInHub)
        }
    }

    /// Saves the user's data to a file on the disk.
    pub async fn save(&self) -> Result {
        tokio::fs::create_dir_all(USER_FOLDER).await?;
        tokio::fs::write(
            format!("{}{:x}", USER_FOLDER, self.id.as_u128()),
            bincode::serialize(self).map_err(|_| DataError::Serialize)?,
        )
        .await?;
        Ok(())
    }

    /// Loads the data of the user with the given ID.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user data file could not be found, probably means that the user does not exist.
    /// * The user data file was corrupt and or could not be deserialized properly.
    pub async fn load(id: &ID) -> Result<Self> {
        let filename = format!("{}{:x}", USER_FOLDER, id.as_u128());
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(Error::UserNotFound);
        }
        bincode::deserialize(&tokio::fs::read(path).await?)
            .map_err(|_| DataError::Deserialize.into())
    }

    /// Same as `Self::load` but first maps an OAuth ID and service name to a WICRS Server ID.
    ///
    /// # Arguments
    ///
    /// * `id` - the ID of the user on their selected OAuth service.
    /// * `service` - the user's selected OAuth service.
    ///
    /// # Errors
    ///
    /// This function will return an error in the same situations outlined by [`User::load`]
    pub async fn load_get_id(id: &str, service: &Service) -> Result<Self> {
        Self::load(&get_id(id, service)).await
    }
}

/// Gets a user ID based on their ID from the OAuth service they used to sign up and the name of the OAuth service.
///
/// # Arguments
///
/// * `id` - the ID of the user on their selected OAuth service.
/// * `service` - the user's selected OAuth service.
pub fn get_id(id: &str, service: &Service) -> ID {
    let mut hasher = Shake128::default();
    hasher.update(id);
    hasher.update(service.to_string());
    let mut bytes = [0; 16];
    hasher
        .finalize_xof()
        .read_exact(&mut bytes)
        .expect("Failed to read the user ID hash");
    ID::from_bytes(bytes)
}
