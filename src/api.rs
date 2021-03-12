use crate::{
    hub::{Hub, HubMember},
    is_valid_username, new_id,
    permission::HubPermission,
    user::{GenericUser, User},
    Error, Result, ID,
};

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

pub async fn delete_hub(user: &User, hub_id: ID) -> Result<()> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if member.has_all_permissions() {
        if let Ok(()) = tokio::fs::remove_file(hub.get_info_path()).await {
            if let Ok(()) = tokio::fs::remove_dir_all(hub.get_data_path()).await {
                Ok(())
            } else {
                Err(Error::DeleteFailed)
            }
        } else {
            Err(Error::DeleteFailed)
        }
    } else {
        Err(Error::NoPermission)
    }
}

pub async fn rename_hub(user: &User, hub_id: ID, new_name: String) -> Result<String> {
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if member.has_permission(HubPermission::Administrate, &hub) {
        let old_name = hub.name.clone();
        hub.name = new_name;
        hub.save().await?;
        Ok(old_name)
    } else {
        Err(Error::NoPermission)
    }
}

pub async fn user_banned(user: &User, hub_id: ID, user_id: ID) -> Result<bool> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    Ok(hub.bans.contains(&user_id))
}

pub async fn user_muted(user: &User, hub_id: ID, user_id: ID) -> Result<bool> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    Ok(hub.mutes.contains(&user_id))
}

pub async fn get_hub_member(user: &User, hub_id: ID, user_id: ID) -> Result<HubMember> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if user.id == user_id {
        return Ok(member);
    } else {
        hub.get_member(&user_id)
    }
}

pub async fn join_hub(user: &mut User, hub_id: ID) -> Result<()> {
    user.join_hub(hub_id).await?;
    user.save().await
}

pub async fn leave_hub(user: &mut User, hub_id: ID) -> Result<()> {
    user.leave_hub(hub_id).await?;
    user.save().await
}

async fn hub_user_op(user: &User, hub_id: ID, user_id: ID, op: HubPermission) -> Result<()> {
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if member.has_permission(op.clone(), &hub) {
        match op {
            HubPermission::Kick => hub.kick_user(user_id).await?,
            HubPermission::Ban => hub.ban_user(user_id).await?,
            HubPermission::Unban => hub.unban_user(user_id),
            HubPermission::Mute => hub.mute_user(user_id),
            HubPermission::Unmute => hub.unmute_user(user_id),
            _ => return Err(Error::UnexpectedServerArg),
        }
        hub.save().await
    } else {
        Err(Error::NoPermission)
    }
}

pub async fn kick_user(user: &User, hub_id: ID, user_id: ID) -> Result<()> {
    hub_user_op(user, hub_id, user_id, HubPermission::Kick).await
}

pub async fn ban_user(user: &User, hub_id: ID, user_id: ID) -> Result<()> {
    hub_user_op(user, hub_id, user_id, HubPermission::Ban).await
}

pub async fn unban_user(user: &User, hub_id: ID, user_id: ID) -> Result<()> {
    hub_user_op(user, hub_id, user_id, HubPermission::Unban).await
}

pub async fn mute_user(user: &User, hub_id: ID, user_id: ID) -> Result<()> {
    hub_user_op(user, hub_id, user_id, HubPermission::Mute).await
}

pub async fn unmute_user(user: &User, hub_id: ID, user_id: ID) -> Result<()> {
    hub_user_op(user, hub_id, user_id, HubPermission::Unmute).await
}

pub async fn create_channel(user: &User, hub_id: ID, name: String) -> Result<ID> {
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let channel_id = hub.new_channel(user.id.clone(), name).await?;
    hub.save().await?;
    Ok(channel_id)
}
