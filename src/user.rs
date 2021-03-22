use std::io::Read;

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator};
use serde::{Deserialize, Serialize};
use sha3::{
    digest::{ExtendableOutput, Update},
    Digest, Sha3_256, Shake128,
};

use crate::{
    auth::Service, check_name_validity, get_system_millis, hub::Hub, ApiError, DataError, Result,
    ID,
};

const USER_FOLDER: &str = "data/users/";

/// Represents a user, keeps track of which accounts it owns and their metadata.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub email: String,
    pub created: u128,
    pub service: Service,
    pub in_hubs: Vec<ID>,
}

/// Represents the publicly available information on a user, (excludes their email address and the service they signed up with) also only includes the generic version of accounts.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericUser {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub hubs_hashed: Vec<String>,
}

impl User {
    /// Creates a new user and generates an ID by hashing the service used and the ID of the user according to that service.
    pub fn new(id: String, email: String, service: Service) -> Self {
        Self {
            id: get_id(&id, &service),
            username: String::new(),
            email,
            service,
            created: get_system_millis(),
            in_hubs: Vec::new(),
        }
    }

    /// Converts the standard user into a GenericUser.
    pub fn to_generic(&self) -> GenericUser {
        let mut hasher = Sha3_256::new();
        let mut hubs_hashed = Vec::new();
        for hub in self.in_hubs.clone() {
            sha3::digest::Update::update(&mut hasher, hub.to_string());
            hubs_hashed.push(format!("{:x}", hasher.finalize_reset()));
        }
        GenericUser {
            id: self.id.clone(),
            created: self.created.clone(),
            username: self.username.clone(),
            hubs_hashed,
        }
    }

    /// Changes the username of the user while checking that it adheres to the rules set by `crate::is_valid_name`.
    pub async fn change_username(&mut self, new_name: String) -> Result<String> {
        check_name_validity(&new_name)?;
        let old_name = self.username.clone();
        self.username = new_name;
        Ok(old_name)
    }

    /// Adds the user to a hub, giving them the default permissions and making sure that they are not banned.
    pub async fn join_hub(&mut self, hub_id: &ID) -> Result<()> {
        let mut hub = Hub::load(hub_id).await?;
        if !hub.bans.contains(&self.id) {
            hub.user_join(&self)?;
            hub.save().await?;
            self.in_hubs.push(hub_id.clone());
            Ok(())
        } else {
            Err(ApiError::Banned)
        }
    }

    /// Returns and error if the user is not in the given hub.
    pub fn in_hub(&self, hub_id: &ID) -> Result<()> {
        if self.in_hubs.contains(hub_id) {
            Ok(())
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Removes the user from the hub.
    pub async fn leave_hub(&mut self, hub_id: &ID) -> Result<()> {
        if let Some(index) = self.in_hubs.par_iter().position_any(|id| id == hub_id) {
            let mut hub = Hub::load(hub_id).await?;
            hub.user_leave(&self)?;
            hub.save().await?;
            self.in_hubs.remove(index);
            Ok(())
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Saves the user's data to a file on the disk.
    pub async fn save(&self) -> Result<()> {
        tokio::fs::create_dir_all(USER_FOLDER).await?;
        let json = serde_json::to_string(self).map_err(|_| DataError::Serialize)?;
        tokio::fs::write(format!("{}{:x}.json", USER_FOLDER, self.id.as_u128()), json).await?;
        Ok(())
    }

    /// Loads the data of a user based on their ID.
    pub async fn load(id: &ID) -> Result<Self> {
        let filename = format!("{}{:x}.json", USER_FOLDER, id.as_u128());
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(ApiError::HubNotFound);
        }
        let json = tokio::fs::read_to_string(path).await?;
        serde_json::from_str(&json).map_err(|_| DataError::Deserialize.into())
    }

    /// Same as `Self::load` but first maps an OAuth ID and service name to a WICRS Server ID.
    pub async fn load_get_id(id: &str, service: &Service) -> Result<Self> {
        Self::load(&get_id(id, service)).await
    }
}

/// Gets a user ID based on their ID from the OAuth service they used to sign up and the name of the OAuth service.
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
