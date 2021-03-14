use crate::{
    auth::{Auth, AuthQuery, IDToken, Service},
    channel::Channel,
    hub::{Hub, HubMember},
    is_valid_username, new_id,
    permission::HubPermission,
    user::{GenericUser, User},
    Error, Result, AUTH, ID,
};

pub async fn start_login(service: Service) -> String {
    Auth::start_login(AUTH.clone(), service).await
}

pub async fn complete_login(service: Service, query: AuthQuery) -> Result<IDToken> {
    Auth::handle_oauth(AUTH.clone(), service, query).await
}

pub async fn invalidate_tokens(user: &User) {
    Auth::invalidate_tokens(AUTH.clone(), user.id).await
}

pub async fn get_user_stripped(id: ID) -> Result<GenericUser> {
    User::load(&id).await.map(User::to_generic).map_err(|_| Error::UserNotFound)
}

pub async fn change_username(user: &mut User, new_name: String) -> Result<String> {
    is_valid_username(&new_name)?;
    let old_name = user.username.clone();
    user.username = new_name;
    user.save().await?;
    Ok(old_name)
}

pub async fn create_hub(owner: &mut User, name: String) -> Result<ID> {
    is_valid_username(&name)?;
    let mut id = new_id();
    while Hub::load(&id).await.is_ok() {
        id = new_id();
    }
    let mut new_hub = Hub::new(name, id.clone(), &owner);
    let channel_id = new_hub.new_channel(&owner.id, "chat".to_string()).await?;
    if let Some(group) = new_hub.groups.get_mut(&new_hub.default_group) {
        group.set_channel_permission(
            channel_id.clone(),
            crate::permission::ChannelPermission::ViewChannel,
            crate::permission::PermissionSetting::TRUE,
        );
        group.set_channel_permission(
            channel_id.clone(),
            crate::permission::ChannelPermission::SendMessage,
            crate::permission::PermissionSetting::TRUE,
        );
        group.set_channel_permission(
            channel_id.clone(),
            crate::permission::ChannelPermission::ReadMessage,
            crate::permission::PermissionSetting::TRUE,
        );
    }
    owner.in_hubs.push(id.clone());
    owner.save().await?;
    new_hub.save().await?;
    Ok(id)
}

pub async fn get_hub(user: &User, hub_id: &ID) -> Result<Hub> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    hub.strip(&user.id)
}

pub async fn delete_hub(user: &User, hub_id: &ID) -> Result<()> {
    user.in_hub(&hub_id)?;
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if member.has_all_permissions() {
        if let Ok(()) = tokio::fs::remove_file(hub.get_info_path()).await {
            tokio::fs::remove_dir_all(hub.get_data_path()).await.map_err(|_| Error::DeleteFailed)
        } else {
            Err(Error::DeleteFailed)
        }
    } else {
        Err(Error::NoPermission)
    }
}

pub async fn rename_hub(user: &User, hub_id: &ID, new_name: String) -> Result<String> {
    is_valid_username(&new_name)?;
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

pub async fn change_nickname(user: &User, hub_id: &ID, new_name: String) -> Result<String> {
    is_valid_username(&new_name)?;
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member_mut(&user.id)?;
    let old_name = member.nickname.clone();
    member.nickname = new_name;
    hub.save().await?;
    Ok(old_name)
}

pub async fn user_banned(user: &User, hub_id: &ID, user_id: &ID) -> Result<bool> {
    user.in_hub(hub_id)?;
    Hub::load(hub_id).await.map(|hub| hub.bans.contains(user_id))
}

pub async fn user_muted(user: &User, hub_id: &ID, user_id: &ID) -> Result<bool> {
    user.in_hub(hub_id)?;
    let hub = Hub::load(hub_id).await?;
    Ok(hub.mutes.contains(user_id))
}

pub async fn get_hub_member(user: &User, hub_id: &ID, user_id: &ID) -> Result<HubMember> {
    user.in_hub(hub_id)?;
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if &user.id == user_id {
        return Ok(member);
    } else {
        hub.get_member(user_id)
    }
}

pub async fn join_hub(user: &mut User, hub_id: &ID) -> Result<()> {
    user.join_hub(hub_id).await?;
    user.save().await
}

pub async fn leave_hub(user: &mut User, hub_id: &ID) -> Result<()> {
    user.leave_hub(hub_id).await?;
    user.save().await
}

async fn hub_user_op(user: &User, hub_id: &ID, user_id: &ID, op: HubPermission) -> Result<()> {
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    if member.has_permission(op.clone(), &hub) {
        match op {
            HubPermission::Kick => hub.kick_user(user_id).await?,
            HubPermission::Ban => hub.ban_user(user_id.clone()).await?,
            HubPermission::Unban => hub.unban_user(user_id),
            HubPermission::Mute => hub.mute_user(user_id.clone()),
            HubPermission::Unmute => hub.unmute_user(user_id),
            _ => return Err(Error::UnexpectedServerArg),
        }
        hub.save().await
    } else {
        Err(Error::NoPermission)
    }
}

macro_rules! action_fns {
  ($(($fnName:ident, $variant:ident)),*) => {
    $(
      pub async fn $fnName(user: &User, hub_id: &ID, user_id: &ID) -> Result<()> {
          hub_user_op(user, hub_id, user_id, HubPermission::$variant).await
      }
    )*
  }
}
action_fns! { 
(kick_user, Kick), 
(ban_user, Ban), 
(unban_user, Unban), 
(mute_user, Mute),
(unmute_user, Unmute)
}

pub async fn create_channel(user: &User, hub_id: &ID, name: String) -> Result<ID> {
    is_valid_username(&name)?;
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let channel_id = hub.new_channel(&user.id, name).await?;
    hub.save().await?;
    Ok(channel_id)
}

pub async fn get_channel(user: &User, hub_id: &ID, channel_id: &ID) -> Result<Channel> {
    user.in_hub(hub_id)?;
    let hub = Hub::load(hub_id).await?;
    Ok(hub.get_channel(&user.id, channel_id)?.clone())
}

pub async fn rename_channel(
    user: &User,
    hub_id: &ID,
    channel_id: &ID,
    new_name: String,
) -> Result<String> {
    is_valid_username(&new_name)?;
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let channel = hub.get_channel_mut(&user.id, channel_id)?;
    let old_name = channel.name.clone();
    channel.name = new_name;
    hub.save().await?;
    Ok(old_name)
}

pub async fn delete_channel(user: &User, hub_id: &ID, channel_id: &ID) -> Result<()> {
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    hub.delete_channel(&user.id, channel_id).await?;
    hub.save().await
}

pub async fn send_message(
    user: &User,
    hub_id: &ID,
    channel_id: &ID,
    message: String,
) -> Result<ID> {
    user.in_hub(hub_id)?;
    if &message.as_bytes().len() < crate::MESSAGE_MAX_SIZE {
        user.send_hub_message(hub_id, channel_id, message).await
    } else {
        Err(Error::MessageTooBig)
    }
}
