use async_graphql::extensions::ApolloTracing;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use serde::{Deserialize, Serialize};
use warp::hyper::body::Bytes;
use xactor::Actor;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use lazy_static::lazy_static;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::graphql_model::GraphQLSchema;
use crate::server::Server;
use crate::ID;
use crate::{api, graphql_model::QueryRoot, server::ServerAddress};
use warp::path;
use warp::Reply;
use warp::{Filter, Rejection};

lazy_static! {
    static ref SERVER_INFO_STRING: String = {
        let server_info_struct = ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        serde_json::to_string(&server_info_struct).unwrap()
    };
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
}

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .extension(ApolloTracing)
        .finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );

    let cors = warp::cors()
        .allow_header("content-type")
        .allow_header("authorization")
        .allow_header("cache-control")
        .allow_any_origin()
        .build();
    let log = warp::log("wicrs_server::httpapi");

    let routes = api(Arc::clone(&server), schema).with(cors).with(log);

    let server = warp::serve(routes).run(
        config
            .address
            .parse::<SocketAddr>()
            .expect("Invalid bind address"),
    );

    server.await;

    Ok(())
}

fn auth() -> impl Filter<Extract = (ID,), Error = warp::Rejection> + Clone {
    warp::header("authorization")
}

fn with_server(
    server: ServerAddress,
) -> impl Filter<Extract = (ServerAddress,), Error = Infallible> + Clone {
    warp::any().map(move || Arc::clone(&server))
}

fn websocket(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("websocket")
        .and(with_server(server))
        .and(auth())
        .and(warp::ws())
        .and_then(api::websocket)
}

fn string_body(max_size: u64) -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(max_size)
        .and(warp::body::bytes())
        .map(|body: Bytes| body.into_iter().collect())
        .and_then(|vector: Vec<u8>| async move {
            Ok::<String, Rejection>(String::from_utf8(vector).map_err(Error::from)?)
        })
}

fn server_info() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("info")
        .map(move || SERVER_INFO_STRING.as_str().to_string())
        .and_then(|server_info: String| async move { Ok::<String, Rejection>(server_info) })
}

fn graphql(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("graphql")
        .and(with_server(server))
        .and(auth())
        .and(async_graphql_warp::graphql(schema))
        .and_then(api::graphql)
}

fn graphql_schema(sdl: String) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("graphql" / "schema")
        .map(move || sdl.clone())
        .and_then(|sdl: String| async move { Ok::<String, Rejection>(sdl) })
}

fn graphql_playground() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("graphql" / "playground" / ID).and_then(|user: ID| async move {
        Ok::<_, Rejection>(
            warp::http::Response::builder()
                .header("content-type", "text/html")
                .body(playground_source(
                    GraphQLPlaygroundConfig::new("/api/v3/graphql")
                        .with_header("authorization", user.to_string().as_str()),
                )),
        )
    })
}

fn rest(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    hub::hub(Arc::clone(&server))
        .or(channel::channel(Arc::clone(&server)))
        .or(member::member(Arc::clone(&server)))
        .or(message::message(Arc::clone(&server)))
}

fn api(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let schema_sdl = schema.sdl();
    path!("api" / "v3" / ..).and(
        rest(Arc::clone(&server))
            .or(websocket(Arc::clone(&server)))
            .or(graphql(server, schema))
            .or(graphql_schema(schema_sdl))
            .or(graphql_playground())
            .or(server_info()),
    )
}

mod hub {
    use super::*;
    fn create() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::path::end()
            .and(warp::post())
            .and(auth())
            .and(string_body(crate::MAX_NAME_SIZE as u64))
            .and_then(api::create_hub)
    }

    fn update(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(api::update_hub)
    }

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_hub)
    }

    fn delete(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::delete())
            .and(auth())
            .and(with_server(server))
            .and_then(api::delete_hub)
    }

    fn join(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / "join")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::join_hub)
    }

    fn leave(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / "leave")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::leave_hub)
    }

    pub fn hub(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("hub" / ..).and(
            join(Arc::clone(&server))
                .or(leave(Arc::clone(&server)))
                .or(get())
                .or(delete(Arc::clone(&server)))
                .or(update(Arc::clone(&server)))
                .or(create()),
        )
    }
}

mod message {
    use super::*;

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / ID)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_message)
    }

    fn get_after() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / ID / usize)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_messages_after)
    }

    fn get_from_to() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / i64 / i64 / usize / bool)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_messages)
    }

    fn send(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::post())
            .and(auth())
            .and(string_body(crate::MESSAGE_MAX_SIZE as u64))
            .and(with_server(server))
            .and_then(api::send_message)
    }

    pub fn message(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("message").and(
            send(Arc::clone(&server))
                .or(get_from_to())
                .or(get_after())
                .or(get()),
        )
    }
}

mod channel {
    use super::*;

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_channel)
    }

    fn create(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::post())
            .and(auth())
            .and(string_body(crate::MAX_NAME_SIZE as u64))
            .and(with_server(server))
            .and_then(api::create_channel)
    }

    fn update(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(api::update_channel)
    }

    fn delete(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::delete())
            .and(auth())
            .and(with_server(server))
            .and_then(api::delete_channel)
    }

    pub fn channel(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("channel" / ..).and(
            get()
                .or(delete(Arc::clone(&server)))
                .or(update(Arc::clone(&server)))
                .or(create(Arc::clone(&server))),
        )
    }
}

mod member {
    use crate::permission::{ChannelPermission, HubPermission, PermissionSetting};

    use super::*;

    fn status() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::get()
            .and(auth())
            .and(path!(ID / ID / "status"))
            .and_then(api::hub_member_status)
    }

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::get()
            .and(auth())
            .and(path!(ID / ID))
            .and_then(api::get_hub_member)
    }

    fn kick(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "kick")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::kick_user)
    }

    fn mute(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "mute")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::mute_user)
    }

    fn ban(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "ban")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::ban_user)
    }

    fn unban(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "unban")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::unban_user)
    }

    fn unmute(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "unmute")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(api::unmute_user)
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    struct PermissionSettingQuery {
        setting: PermissionSetting,
    }

    fn set_hub_permission(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "hub_permission" / HubPermission)
            .and(warp::put())
            .and(auth())
            .and(warp::query().map(|s: PermissionSettingQuery| s.setting))
            .and(with_server(server))
            .and_then(api::set_member_hub_permission)
    }

    fn get_hub_permission() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "hub_permission" / HubPermission)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_member_hub_permission)
    }

    fn set_channel_permission(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "channel_permission" / ID / ChannelPermission)
            .and(warp::put())
            .and(auth())
            .and(warp::query().map(|s: PermissionSettingQuery| s.setting))
            .and(with_server(server))
            .and_then(api::set_member_channel_permission)
    }

    fn get_channel_permission() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "channel_permission" / ID / ChannelPermission)
            .and(warp::get())
            .and(auth())
            .and_then(api::get_member_channel_permission)
    }

    pub fn member(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("channel" / ..).and(
            get()
                .or(status())
                .or(kick(Arc::clone(&server)))
                .or(mute(Arc::clone(&server)))
                .or(ban(Arc::clone(&server)))
                .or(unmute(Arc::clone(&server)))
                .or(unban(Arc::clone(&server)))
                .or(get_hub_permission())
                .or(set_hub_permission(Arc::clone(&server)))
                .or(get_channel_permission())
                .or(set_channel_permission(Arc::clone(&server))),
        )
    }
}
