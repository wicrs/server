use std::mem;

use chrono::{DateTime, Utc};

use crate::{
    channel::{Channel, Message},
    check_name_validity, check_permission,
    error::Error,
    hub::{Hub, HubMember},
    new_id,
    permission::{ChannelPermission, HubPermission, PermissionSetting},
    Result, ID,
};

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
pub async fn create_hub<S: Into<String>>(owner_id: ID, name: S) -> Result<ID> {
    let name: String = name.into();
    check_name_validity(&name)?;
    let mut id = new_id();
    while Hub::load(id).await.is_ok() {
        id = new_id();
    }
    let mut new_hub = Hub::new(name, id, owner_id);
    let channel_id = new_hub.new_channel(&owner_id, "chat".to_string()).await?;
    if let Some(group) = new_hub.groups.get_mut(&new_hub.default_group) {
        group.set_channel_permission(
            channel_id,
            crate::permission::ChannelPermission::Read,
            Some(true),
        );
        group.set_channel_permission(
            channel_id,
            crate::permission::ChannelPermission::Write,
            Some(true),
        );
    }
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
pub async fn get_hub(user_id: &ID, hub_id: ID) -> Result<Hub> {
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
pub async fn delete_hub(user_id: &ID, hub_id: ID) -> Result {
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
    hub_id: ID,
    new_name: S,
) -> Result<String> {
    let new_name: String = new_name.into();
    check_name_validity(&new_name)?;
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(user_id)?;
    check_permission!(member, HubPermission::Administrate, hub);
    let old_name = mem::replace(&mut hub.name, new_name);
    hub.save().await?;
    Ok(old_name)
}

/// Changes the description of a hub.
///
/// # Arguments
///
/// * `user_id` - ID of the user to check for permission to perform the operation.
/// * `hub_id` - The ID of the hub whose name is to be changed.
/// * `new_description` - The content for the hub's description
///
/// # Errors
///
/// This function may return an error for any of the following reasons:
///
/// * THe user is not in the hub.
/// * The user does not have permission to change the hub's description.
/// * The given description is bigger than [`crate::MAX_DESCRIPTION_SIZE`].
/// * The hub could not be loaded for any of the reasons outlined by [`Hub::load`].
/// * The hub could not be saved for any of the reasons outlined by [`Hub::save`].
pub async fn change_hub_description<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: ID,
    new_description: S,
) -> Result<String> {
    let new_description: String = new_description.into();
    if new_description.as_bytes().len() > crate::MAX_DESCRIPTION_SIZE {
        Err(Error::TooBig)
    } else {
        let mut hub = Hub::load(hub_id).await?;
        let member = hub.get_member(user_id)?;
        check_permission!(member, HubPermission::Administrate, hub);
        let old_name = mem::replace(&mut hub.description, new_description);
        hub.save().await?;
        Ok(old_name)
    }
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
pub async fn user_banned(actor_id: &ID, hub_id: ID, user_id: &ID) -> Result<bool> {
    let hub = Hub::load(hub_id).await?;
    hub.check_membership(actor_id)?;
    Ok(hub.bans.contains(user_id))
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
pub async fn user_muted(actor_id: &ID, hub_id: ID, user_id: &ID) -> Result<bool> {
    let hub = Hub::load(hub_id).await?;
    hub.check_membership(actor_id)?;
    Ok(hub.mutes.contains(user_id))
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
pub async fn get_hub_member(actor_id: &ID, hub_id: ID, user_id: &ID) -> Result<HubMember> {
    let hub = Hub::load(hub_id).await?;
    hub.check_membership(actor_id)?;
    Ok(hub.get_member(user_id)?.clone())
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
pub async fn join_hub(user_id: ID, hub_id: ID) -> Result {
    let mut hub = Hub::load(hub_id).await?;
    hub.user_join(user_id)?;
    hub.save().await
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
pub async fn leave_hub(user_id: &ID, hub_id: ID) -> Result {
    let mut hub = Hub::load(hub_id).await?;
    hub.user_leave(user_id)?;
    hub.save().await
}

/// Handles kicking, banning, muting, unbanning and unmuting users in/from hubs.
async fn hub_user_op(actor_id: &ID, hub_id: ID, user_id: ID, op: HubPermission) -> Result {
    let mut hub = Hub::load(hub_id).await?;
    let member = hub.get_member(actor_id)?;
    check_permission!(member, op, hub);
    match op {
        HubPermission::Kick => hub.kick_user(&user_id)?,
        HubPermission::Ban => hub.ban_user(user_id)?,
        HubPermission::Unban => hub.unban_user(&user_id),
        HubPermission::Mute => hub.mute_user(user_id),
        HubPermission::Unmute => hub.unmute_user(&user_id),
        _ => return Err(Error::UnexpectedServerArg),
    }
    hub.save().await
}

/// Maps the different possible options for [`hub_user_op`] to separate functions.
macro_rules! action_fns {
  ($($(#[$attr:meta])* => ($fnName:ident, $variant:ident)),*) => {
    $(
      $(#[$attr])*
      pub async fn $fnName(actor_id: &ID, hub_id: ID, user_id: ID) -> Result<()> {
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
    hub_id: ID,
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
pub async fn get_channel(user_id: &ID, hub_id: ID, channel_id: ID) -> Result<Channel> {
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
    hub_id: ID,
    channel_id: ID,
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
pub async fn change_channel_description<S: Into<String> + Clone>(
    user_id: &ID,
    hub_id: ID,
    channel_id: ID,
    new_description: S,
) -> Result<String> {
    let description: String = new_description.into();
    if description.as_bytes().len() > crate::MAX_DESCRIPTION_SIZE {
        Err(Error::TooBig)
    } else {
        let mut hub = Hub::load(hub_id).await?;
        let old_name = hub
            .change_channel_description(user_id, channel_id, description)
            .await?;
        hub.save().await?;
        Ok(old_name)
    }
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
pub async fn delete_channel(user_id: &ID, hub_id: ID, channel_id: ID) -> Result {
    let mut hub = Hub::load(hub_id).await?;
    hub.delete_channel(user_id, channel_id).await?;
    hub.save().await
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
    hub_id: ID,
    channel_id: ID,
    message_id: ID,
) -> Result<Message> {
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, user_id, channel_id)?;
    if let Some(message) = channel.get_message(message_id).await {
        Ok(message)
    } else {
        Err(Error::MessageNotFound)
    }
}

/// Gets messages sent after a given message.
/// If successful they are returned in an array. The array is orderd oldest message to newest
/// If there are no messages after the given message or the given message is not found, an empty array is returned.
///
/// # Arguments
///
/// * `user_id` - ID of the user who is requesting the message.
/// * `hub_id` - ID of the hub where the message is located.
/// * `channel_id` - ID of the channel where the message is located.
/// * `from` - ID of the message to start from.
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
pub async fn get_messages_after(
    user_id: &ID,
    hub_id: ID,
    channel_id: ID,
    from: ID,
    max: usize,
) -> Result<Vec<Message>> {
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, user_id, channel_id)?;
    Ok(channel.get_messages_after(from, max).await)
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
    hub_id: ID,
    channel_id: ID,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    invert: bool,
    max: usize,
) -> Result<Vec<Message>> {
    let hub = Hub::load(hub_id).await?;
    let channel = Hub::get_channel(&hub, user_id, channel_id)?;
    Ok(channel.get_messages_between(from, to, invert, max).await)
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
    hub_id: ID,
    member_id: &ID,
    permission: HubPermission,
    value: PermissionSetting,
) -> Result {
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
    hub_id: ID,
    member_id: &ID,
    channel_id: ID,
    permission: ChannelPermission,
    value: PermissionSetting,
) -> Result {
    let mut hub = Hub::load(hub_id).await?;
    {
        let member = hub.get_member(user_id)?;
        check_permission!(member, HubPermission::Administrate, hub);
    }
    let member = hub.get_member_mut(member_id)?;
    member.set_channel_permission(channel_id, permission, value);
    hub.save().await
}
