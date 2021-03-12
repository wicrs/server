use crate::{Error, ID, Result, is_valid_username, new_id, user::{GenericUser, User}, hub::Hub};

pub async fn get_user_stripped(id: ID) -> Result<GenericUser> {
    if let Ok(other) = User::load(&id).await {
        Ok(other.to_generic())
    } else {
        Err(Error::UserNotFound)
    }
}

pub async fn create_hub(name: String, owner: &mut User) -> Result<ID> {
    is_valid_username(&name)?;
    let mut id = new_id();
    while Hub::load(id).await.is_ok() {
        id = new_id();
    }
    let new_hub = Hub::new(name, id.clone(), &owner);
    new_hub.save().await?;
    owner.in_hubs.push(id.clone());
    owner.save().await?;
    Ok(id)
}

pub async fn get_hub(user: &User, hub_id: ID) -> Result<Hub> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    hub.strip(user.id)
}
