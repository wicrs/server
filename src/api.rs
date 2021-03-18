use crate::{AUTH, ApiError, ID, Result, auth::{Auth, AuthQuery, IDToken, Service}, channel::{Channel, Message}, check_name_validity, check_permission, get_system_millis, hub::{Hub, HubMember}, new_id, permission::HubPermission, user::{GenericUser, User}};

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
    User::load(&id)
        .await
        .map(|u| User::to_generic(&u))
        .map_err(|_| ApiError::UserNotFound)
}

pub async fn change_username<S: Into<String> + Clone>(
    user: &mut User,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    let old_name = user.username.clone();
    user.username = new_name.into();
    user.save().await?;
    Ok(old_name)
}

pub async fn create_hub(owner: &mut User, name: String) -> Result<ID> {
    check_name_validity(&name)?;
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
            Some(true),
        );
        group.set_channel_permission(
            channel_id.clone(),
            crate::permission::ChannelPermission::SendMessage,
            Some(true),
        );
        group.set_channel_permission(
            channel_id.clone(),
            crate::permission::ChannelPermission::ReadMessage,
            Some(true),
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
    check_permission!(member, HubPermission::All, hub);
    tokio::fs::remove_file(hub.get_info_path()).await?;
    tokio::fs::remove_dir_all(hub.get_data_path()).await?;
    Ok(())
}

pub async fn rename_hub<S: Into<String> + Clone>(
    user: &User,
    hub_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(&user.id)?;
    check_permission!(member, HubPermission::Administrate, hub);
    let old_name = hub.name.clone();
    hub.name = new_name.into();
    hub.save().await?;
    Ok(old_name)
}

pub async fn change_nickname<S: Into<String> + Clone>(
    user: &User,
    hub_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    user.in_hub(&hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member_mut(&user.id)?;
    let old_name = member.nickname.clone();
    member.nickname = new_name.into();
    hub.save().await?;
    Ok(old_name)
}

pub async fn user_banned(user: &User, hub_id: &ID, user_id: &ID) -> Result<bool> {
    user.in_hub(hub_id)?;
    Hub::load(hub_id)
        .await
        .map(|hub| hub.bans.contains(user_id))
}

pub async fn user_muted(user: &User, hub_id: &ID, user_id: &ID) -> Result<bool> {
    user.in_hub(hub_id)?;
    Hub::load(hub_id)
        .await
        .map(|hub| hub.mutes.contains(user_id))
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
    check_permission!(member, op, hub);
    match op {
        HubPermission::Kick => hub.kick_user(user_id).await?,
        HubPermission::Ban => hub.ban_user(user_id.clone()).await?,
        HubPermission::Unban => hub.unban_user(user_id),
        HubPermission::Mute => hub.mute_user(user_id.clone()),
        HubPermission::Unmute => hub.unmute_user(user_id),
        _ => return Err(ApiError::UnexpectedServerArg),
    }
    hub.save().await
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

pub async fn create_channel<S: Into<String> + Clone>(
    user: &User,
    hub_id: &ID,
    name: S,
) -> Result<ID> {
    check_name_validity(&name.clone().into())?;
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let channel_id = hub.new_channel(&user.id, name.into()).await?;
    hub.save().await?;
    Ok(channel_id)
}

pub async fn get_channel(user: &User, hub_id: &ID, channel_id: &ID) -> Result<Channel> {
    user.in_hub(hub_id)?;
    let hub = Hub::load(hub_id).await?;
    Ok(hub.get_channel(&user.id, channel_id)?.clone())
}

pub async fn rename_channel<S: Into<String> + Clone>(
    user: &User,
    hub_id: &ID,
    channel_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    user.in_hub(hub_id)?;
    let mut hub = Hub::load(hub_id).await?;
    let channel = hub.get_channel_mut(&user.id, channel_id)?;
    let old_name = channel.name.clone();
    channel.name = new_name.into();
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
    if message.as_bytes().len() < crate::MESSAGE_MAX_SIZE {
        user.send_hub_message(hub_id, channel_id, message).await
    } else {
        Err(ApiError::MessageTooBig)
    }
}

pub async fn get_message(
    user: &User,
    hub_id: &ID,
    channel_id: &ID,
    message_id: &ID,
) -> Result<Message> {
    user.in_hub(hub_id)?;
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, &user.id, channel_id)?;
    if let Some(message) = channel.get_message(message_id).await {
        Ok(message)
    } else {
        Err(ApiError::MessageNotFound)
    }
}

pub async fn get_messages(
    user: &User,
    hub_id: &ID,
    channel_id: &ID,
    from: Option<u128>,
    to: Option<u128>,
    invert: Option<bool>,
    max: Option<usize>,
) -> Result<Vec<Message>> {
    user.in_hub(hub_id)?;
    let time = get_system_millis();
    let invert = invert.unwrap_or(false);
    let max = max.unwrap_or(100);
    let from = from.unwrap_or(time - 86400001);
    let to = to.unwrap_or(time);
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, &user.id, channel_id)?;
    Ok(channel.get_messages(from, to, invert, max).await)
}
