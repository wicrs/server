use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    auth::{Auth, AuthQuery, IDToken, Service},
    channel::{Channel, Message},
    check_name_validity, check_permission,
    error::Error,
    hub::{Hub, HubMember},
    new_id,
    permission::{ChannelPermission, HubPermission, PermissionSetting},
    user::{GenericUser, User},
    Result, ID,
};

/// Start the OAuth login process. Returns a redirect to the given OAuth service's page with the correct parameters.
///
/// # Arguments
///
/// * `auth_manager` - The Authentication manager for the current server instance, wrapped in Arc<Lock>> so that it can be used by multiple threads.
/// * `service` - The OAuth service to use for this login attempt.
pub async fn start_login(auth_manager: Arc<RwLock<Auth>>, service: Service) -> String {
    Auth::start_login(auth_manager, service).await
}

/// Completes the OAuth login request.
///
/// # Arguments
///
/// * `auth_manager` - The Authentication manager for the current server instance, wrapped in Arc<RwLock>> so that it can be used by multiple threads.
/// * `service` - The OAuth service used in the [`start_login`] step.
/// * `query` - The OAuth query containing the `state` string and the OAuth `code` as well as an optional expiry time.
///
/// # Errors
///
/// This function may return an error for any of the reasons outlined in [`Auth::handle_oauth`].
pub async fn complete_login(
    auth_manager: Arc<RwLock<Auth>>,
    service: Service,
    query: AuthQuery,
) -> Result<IDToken> {
    Auth::handle_oauth(auth_manager, service, query).await
}

/// Invalidates all of a user's authentication token sessions.
///
/// # Arguments
///
/// * `auth_manager` - The Authentication manager for the current server instance, wrapped in Arc<RwLock>> so that it can be used by multiple threads.
/// * `user_id` - ID of the user whose tokens should be invalidated.
pub async fn invalidate_tokens(auth_manager: Arc<RwLock<Auth>>, user_id: ID) {
    Auth::invalidate_tokens(auth_manager, user_id).await
}

/// Gets a user's data while removing all of their private information.
///
/// # Arguments
///
/// * `id` - The id of the user whose data is being requested.
/// * `user_id` - ID of the user who is requesting the data.
///
/// # Errors
///
/// This function may return an error if the user's data cannot be loaded for any of the reasons outlined in [`User::load`].
pub async fn get_user_stripped(user_id: &ID, id: ID) -> Result<GenericUser> {
    User::load(&id)
        .await
        .map(|u| User::to_generic(&u, user_id))
        .map_err(|_| Error::UserNotFound)
}

/// Changes a user's username.
/// Returns the user's previous username if successful.
///
/// # Arguments
///
/// * `user` - User whose name is to be changed.
/// * `new_name` - New username for the user.
///
/// # Errors
///
/// This function may return an error for any of the following reasons.
///
/// * The modified user data could not be saved for any of the reasons outlined in [`User::save`].
/// * The user's name could not be changed for any of the reasons outlined in [`User::change_username`].
pub async fn change_username<S: Into<String> + Clone>(user_id: &ID, new_name: S) -> Result<String> {
    let name: String = new_name.into();
    check_name_validity(&name)?;
    let mut user = User::load(user_id).await?;
    let old_name = user.username;
    user.username = name;
    user.save().await?;
    Ok(old_name)
}

/// Creates a hub, returning the ID of the new hub if successful.
/// Also adds a default channel named "chat" that all users have access to by default.
///
/// # Arguments
///
/// * `owner_id` - ID of the user who should be marked as the owner/creator of the hub.
/// * `name` - The name of the new hub.
///
/// # Errors
///
/// This function may return an error for any of the reasons outlined in the following functions:
///
/// * The user's data could not be saved for any of the reasons outlined in [`User::save`].
/// * The hub failed to save for any of the reasons outlined in [`Hub::save`].
/// * The given name failed to pass the checks for any of the reasons outlined in [`check_name_validity`].
/// * The default channel could not be created for any of the reaons outlined in [`Hub::new_channel`].
pub async fn create_hub(owner_id: &ID, name: String) -> Result<ID> {
    check_name_validity(&name)?;
    let mut id = new_id();
    while Hub::load(&id).await.is_ok() {
        id = new_id();
    }
    let mut owner = User::load(owner_id).await?;
    let mut new_hub = Hub::new(name, id.clone(), &owner);
    let channel_id = new_hub.new_channel(owner_id, "chat".to_string()).await?;
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

/// Gets a hub stripped of data the given user should not be able to see.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check the visiblity of objects for.
/// * `hub_id` - ID of the hub to get.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The hub failed to load for any of the reasons outlined in [`Hub::load`].
pub async fn get_hub(user_id: &ID, hub_id: &ID) -> Result<Hub> {
    let hub = Hub::load(hub_id).await?;
    hub.strip(user_id)
}

/// Deletes a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to perform the operation.
/// * `hub_id` - ID of the hub to delete.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user does not have permission to delete the hub.
/// * The hub's data files could not be deleted.
pub async fn delete_hub(user_id: &ID, hub_id: &ID) -> Result<()> {
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(user_id)?;
    check_permission!(member, HubPermission::All, hub);
    tokio::fs::remove_file(hub.get_info_path()).await?;
    tokio::fs::remove_dir_all(hub.get_data_path()).await?;
    Ok(())
}

/// Changes the name of a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to perform the operation.
/// * `hub_id` - The ID of the hub whose name is to be changed.
/// * `new_name` - The new name to be given to the hub.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The user does not have permission to rename the hub.
/// * The given name failed to pass the checks for any of the reasons outlined in [`check_name_validity`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
pub async fn rename_hub<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(user_id)?;
    check_permission!(member, HubPermission::Administrate, hub);
    let old_name = hub.name.clone();
    hub.name = new_name.into();
    hub.save().await?;
    Ok(old_name)
}

/// Changes a user's nickname in a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user whose nickname is to be changed.
/// * `hub_id` - ID of the hub where the new nickname should be applied.
/// * `new_name` - New nickname to be used.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The new name failed to pass the checks for any of the reasons outlined in [`check_name_validity`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
pub async fn change_nickname<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member_mut(user_id)?;
    let old_name = member.nickname.clone();
    member.nickname = new_name.into();
    hub.save().await?;
    Ok(old_name)
}

/// Checks if a user is banned from a hub.
/// Returns `true` if they are and `false` if they aren't.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is checking.
/// * `hub_id` - The hub in which to check the ban status.
/// * `user_id` - The user whose ban status is to be checked.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user who is checking is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn user_banned(actor_id: &ID, hub_id: &ID, user_id: &ID) -> Result<bool> {
    let hub = Hub::load(hub_id).await?;
    if hub.members.contains_key(actor_id) {
        Ok(hub.bans.contains(user_id))
    } else {
        Err(Error::NotInHub)
    }
}

/// Checks if a user is muted in a hub.
/// Returns `true` if they are and `false` if they aren't.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is checking.
/// * `hub_id` - The hub in which to check the mute status.
/// * `user_id` - The user whose mute status is to be checked.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user who is checking is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn user_muted(actor_id: &ID, hub_id: &ID, user_id: &ID) -> Result<bool> {
    let hub = Hub::load(hub_id).await?;
    if hub.members.contains_key(actor_id) {
        Ok(hub.mutes.contains(user_id))
    } else {
        Err(Error::NotInHub)
    }
}

/// Gets the information on a member of a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is requesting the information.
/// * `hub_id` - Hub from which to get the information.
/// * `user_id` - ID of the user whose information is being requested.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The requesting user is not in the hub.
/// * The user whose information is being requested is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn get_hub_member(actor_id: &ID, hub_id: &ID, user_id: &ID) -> Result<HubMember> {
    let hub = Hub::load(hub_id).await?;
    let member = hub.get_member(actor_id)?;
    if actor_id == user_id {
        return Ok(member);
    } else {
        hub.get_member(user_id)
    }
}

/// Adds the given user to a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to add to the hub.
/// * `hub_id` - ID of the hub the user is to be added to.
///
/// # Errors
///
/// * The user could not be added to the hub for any of the reasons outlined by [`User::join_hub`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
pub async fn join_hub(user_id: &ID, hub_id: &ID) -> Result<()> {
    let mut user = User::load(user_id).await?;
    user.join_hub(hub_id).await?;
    user.save().await
}

/// Removes the given user from a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to remove from the hub.
/// * `hub_id` - ID of the hub the user is to be removed from.
///
/// # Errors
///
/// * The user could not be removed from the hub for any of the reasons outlined by [`User::leave_hub`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
pub async fn leave_hub(user_id: &ID, hub_id: &ID) -> Result<()> {
    let mut user = User::load(user_id).await?;
    user.leave_hub(hub_id).await?;
    user.save().await
}

async fn hub_user_op(actor_id: &ID, hub_id: &ID, user_id: &ID, op: HubPermission) -> Result<()> {
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(actor_id)?;
    check_permission!(member, op, hub);
    match op {
        HubPermission::Kick => hub.kick_user(user_id).await?,
        HubPermission::Ban => hub.ban_user(user_id.clone()).await?,
        HubPermission::Unban => hub.unban_user(user_id),
        HubPermission::Mute => hub.mute_user(user_id.clone()),
        HubPermission::Unmute => hub.unmute_user(user_id),
        _ => return Err(Error::UnexpectedServerArg),
    }
    hub.save().await
}

macro_rules! action_fns {
  ($($(#[$attr:meta])* => ($fnName:ident, $variant:ident)),*) => {
    $(
      $(#[$attr])*
      pub async fn $fnName(actor_id: &ID, hub_id: &ID, user_id: &ID) -> Result<()> {
          hub_user_op(actor_id, hub_id, user_id, HubPermission::$variant).await
      }
    )*
  }
}
action_fns! {
/// Kicks a user from a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is doing the kicking.
/// * `hub_id` - Hub from which the user is being kicked.
/// * `user_id` - ID of the user who is to be kicked.
///
/// # Errors
///
/// This function may fail for any of the following reasons:
///
/// * The user doing the kicking is not in the hub
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user to be kicked is not in the hub.
/// * The user doing the kicking does not have permission to kick other users.
/// * The kick failed for any of the reasons outlined by [`Hub::kick_user`].
=> (kick_user, Kick),
/// Bans a user from a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is performing the ban.
/// * `hub_id` - Hub from which the user is being banned.
/// * `user_id` - ID of the user who is to be banned.
///
/// # Errors
///
/// This function may fail for any of the following reasons:
///
/// * The user performing the ban is not in the hub
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user performing the ban does not have permission to ban other users.
/// * The ban failed for any of the reasons outlined by [`Hub::ban_user`].
=> (ban_user, Ban),
/// Unbans a user from a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is unbanning.
/// * `hub_id` - Hub from which the user is being unbanned.
/// * `user_id` - ID of the user who is to be unbanned.
///
/// # Errors
///
/// This function may fail for any of the following reasons:
///
/// * The user performing the unban is not in the hub
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user doing the unban does not have permission to unban other users.
/// * The unban failed for any of the reasons outlined by [`Hub::unban_user`].
=> (unban_user, Unban),
/// Mutes a user in a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is muting.
/// * `hub_id` - Hub in which the user is being muted.
/// * `user_id` - ID of the user who is to be muted.
///
/// # Errors
///
/// This function may fail for any of the following reasons:
///
/// * The user performing the mute is not in the hub
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user performing the mute does not have permission to mute other users.
/// * The mute failed for any of the reasons outlined by [`Hub::mute_user`].
=> (mute_user, Mute),
/// Unmutes a user in a hub.
///
/// # Arguments
///
/// * `actor_id` - ID of the user who is unmuting.
/// * `hub_id` - Hub in which the user is being unmuted.
/// * `user_id` - ID of the user who is to be unmuted.
///
/// # Errors
///
/// This function may fail for any of the following reasons:
///
/// * The user performing the unmute is not in the hub
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The user performing the unmute does not have permission to unmute other users.
/// * The unmute failed for any of the reasons outlined by [`Hub::unmute_user`].
=> (unmute_user, Unmute)
}

/// Creates a text channel in a hub.
/// Returns the ID of the new channel if successful.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to create the channel.
/// * `hub_id` - ID of the hub in which the channel should be created.
/// * `name` - Name for the new channel.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The name failed to pass the checks for any of the reasons outlined in [`check_name_validity`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
/// * The user does not have permission to create new channels.
/// * The channel could not be created for any of the reasons outlined by [`Hub::new_channel`].
pub async fn create_channel<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: &ID,
    name: S,
) -> Result<ID> {
    check_name_validity(&name.clone().into())?;
    let mut hub = Hub::load(hub_id).await?;
    let channel_id = hub.new_channel(user_id, name.into()).await?;
    hub.save().await?;
    Ok(channel_id)
}

/// Gets a channel's information.
///
/// # Arguments
///
/// * `user_id` - ID of the user that is requesting the information.
/// * `hub_id` - ID of the hub the channel is in.
/// * `channel_id` - ID of the channel to get.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The channel does not exist.
pub async fn get_channel(user_id: &ID, hub_id: &ID, channel_id: &ID) -> Result<Channel> {
    let hub = Hub::load(hub_id).await?;
    Ok(hub.get_channel(user_id, channel_id)?.clone())
}

/// Renames a text channel in a hub.
/// Returns the previous name of the channel if successful.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to rename the channel.
/// * `hub_id` - ID of the hub that has the channel.
/// * `channel_id` - ID of the channel to be renamed.
/// * `new_name` - New name for the channel.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The name failed to pass the checks for any of the reasons outlined in [`check_name_validity`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
/// * The user does not have permission to rename channels.
/// * The channel could not be renamed for any of the reasons outlined by [`Hub::rename_channel`].
pub async fn rename_channel<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: &ID,
    channel_id: &ID,
    new_name: S,
) -> Result<String> {
    check_name_validity(&new_name.clone().into())?;
    let mut hub = Hub::load(hub_id).await?;
    let old_name = hub
        .rename_channel(user_id, channel_id, new_name.into())
        .await?;
    hub.save().await?;
    Ok(old_name)
}

/// Deletes a text channel in a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to delete channels.
/// * `hub_id` - ID of the hub that has the channel.
/// * `channel_id` - ID of the channel to be deleted.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
/// * The user does not have permission to delete channels.
/// * The channel could not be deleted for any of the reasons outlined by [`Hub::delete_channel`].
pub async fn delete_channel(user_id: &ID, hub_id: &ID, channel_id: &ID) -> Result<()> {
    let mut hub = Hub::load(hub_id).await?;
    hub.delete_channel(user_id, channel_id).await?;
    hub.save().await
}

/// Sends a message in a text channel in a hub.
/// Returns the message if successful.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is sending the message.
/// * `hub_id` - ID of the hub where the message is being sent.
/// * `channel_id` - ID of the channel where the message is being sent.
/// * `message` - The actual message to be sent.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The message is too big (maximum size in bytes is determined by [`crate::MESSAGE_MAX_SIZE`]).
/// * The message could not be sent for any of the reasons outlined by [`Hub::send_message`].
/// * The channel could not be gotten for any of the reasons outlined by [`Hub::get_channel`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn send_message(
    user_id: &ID,
    hub_id: &ID,
    channel_id: &ID,
    message: String,
) -> Result<Message> {
    if message.as_bytes().len() < crate::MESSAGE_MAX_SIZE {
        let mut hub = Hub::load(hub_id).await?;
        hub.send_message(user_id, channel_id, message).await
    } else {
        Err(Error::MessageTooBig)
    }
}

/// Gets a message from a text channel in a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is requesting the message.
/// * `hub_id` - ID of the hub where the message is located.
/// * `channel_id` - ID of the channel where the message is located.
/// * `message_id` - ID of the message to retreive.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The channel could not be found in the hub.
/// * The message could not be found.
/// * The channel could not be gotten for any of the reasons outlined by [`Hub::get_channel`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn get_message(
    user_id: &ID,
    hub_id: &ID,
    channel_id: &ID,
    message_id: &ID,
) -> Result<Message> {
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, user_id, channel_id)?;
    if let Some(message) = channel.get_message(message_id).await {
        Ok(message)
    } else {
        Err(Error::MessageNotFound)
    }
}

/// Gets a set of messages between two times (both in milliseconds since Unix Epoch).
/// If successful they are returned in an array. The array is orderd oldest message to newest
/// unless the `invert` argument is `true` in which case the order is newest to oldest message.
/// If there are no messages in the given time frame, an empty array is returned.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is requesting the message.
/// * `hub_id` - ID of the hub where the message is located.
/// * `channel_id` - ID of the channel where the message is located.
/// * `from` - Earliest time a message can be sent to be included in the results.
/// * `to` - Latest time a message can be sent to be included in the results.
/// * `invert` - If true the search is done from newest message to oldest message, if false the search is done from oldest message to newest message.
/// * `max` - The maximum number of messages to retreive.
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * The user is not in the hub.
/// * The channel could not be found in the hub.
/// * The channel could not be gotten for any of the reasons outlined by [`Hub::get_channel`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn get_messages(
    user_id: &ID,
    hub_id: &ID,
    channel_id: &ID,
    from: u128,
    to: u128,
    invert: bool,
    max: usize,
) -> Result<Vec<Message>> {
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, user_id, channel_id)?;
    Ok(channel.get_messages(from, to, invert, max).await)
}

/// Sets a hub wide permission for a hub member.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is making the change.
/// * `hub_id` - The hub in which the change is being made.
/// * `member_id` - The hub member whose permissions are being changed.
/// * `permission` - The permission whose setting is being changed.
/// * `value` - The new setting for the permission.
///
/// # Errors
///
/// This function may return an error for any of the following reasons.
///
/// * The user making the change is not in the hub.
/// * The user whose permission is being changed is not in the hub.
/// * The user making the change does not have permission to do so.
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn set_member_hub_permission(
    user_id: &ID,
    hub_id: &ID,
    member_id: &ID,
    permission: HubPermission,
    value: PermissionSetting,
) -> Result<()> {
    let mut hub = Hub::load(hub_id).await?;
    {
        let member = hub.get_member(user_id)?;
        check_permission!(member, HubPermission::Administrate, hub);
    }
    let member = hub.get_member_mut(member_id)?;
    member.set_permission(permission, value);
    hub.save().await
}

/// Sets a channel specific permission for a hub member.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is making the change.
/// * `hub_id` - The hub in which the change is being made.
/// * `member_id` - The hub member whose permissions are being changed.
/// * `channel_id` - The channel that the change should apply to.
/// * `permission` - The permission whose setting is being changed.
/// * `value` - The new setting for the permission.
///
/// # Errors
///
/// This function may return an error for any of the following reasons.
///
/// * The user making the change is not in the hub.
/// * The user whose permission is being changed is not in the hub.
/// * The user making the change does not have permission to do so.
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
pub async fn set_member_channel_permission(
    user_id: &ID,
    hub_id: &ID,
    member_id: &ID,
    channel_id: &ID,
    permission: ChannelPermission,
    value: PermissionSetting,
) -> Result<()> {
    let mut hub = Hub::load(hub_id).await?;
    {
        let member = hub.get_member(user_id)?;
        check_permission!(member, HubPermission::Administrate, hub);
    }
    let member = hub.get_member_mut(member_id)?;
    member.set_channel_permission(channel_id, permission, value);
    hub.save().await
}
