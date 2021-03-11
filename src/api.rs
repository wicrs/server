use crate::{Error, ID, user::{GenericUser, User}};

pub async fn get_user_stripped(id: ID) -> Result<GenericUser, Error> {
    if let Ok(other) = User::load(&id).await {
        Ok(other.to_generic())
    } else {
        Err(Error::UserNotFound)
    }
}
