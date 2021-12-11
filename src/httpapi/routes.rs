use async_graphql::extensions::ApolloTracing;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use std::convert::Infallible;
use std::sync::Arc;

use lazy_static::lazy_static;

use crate::error::{ApiError, Error};
use crate::graphql_model::GraphQLSchema;
use crate::httpapi::handlers;
use crate::prelude::{HttpServerInfo, HttpSetPermission};
use crate::ID;
use crate::{graphql_model::QueryRoot, server::ServerAddress};
use warp::http::Method;
use warp::path;
use warp::Reply;
use warp::{Filter, Rejection};

lazy_static! {
    static ref SERVER_INFO_STRING: String = {
        let server_info_struct = HttpServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        serde_json::to_string(&server_info_struct).unwrap()
    };
}

pub fn routes(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .extension(ApolloTracing)
        .finish();

    let cors = warp::cors()
        .allow_header("content-type")
        .allow_header("authorization")
        .allow_header("cache-control")
        .allow_methods([Method::GET, Method::PUT, Method::POST, Method::DELETE])
        .allow_any_origin()
        .build();
    let log = warp::log("wicrs_server::httpapi");

    api(server, schema)
        .recover(handle_rejection)
        .with(log)
        .with(cors)
}

fn api(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let schema_sdl = schema.sdl();
    path!("api" / ..).and(
        rest(Arc::clone(&server))
            .or(websocket(Arc::clone(&server)))
            .or(graphql(server, schema))
            .or(graphql_schema(schema_sdl))
            .or(graphql_playground())
            .or(server_info()),
    )
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    if err.is_not_found() {
        Ok(ApiError::NotFound.into_response())
    } else if let Some(e) = err.find::<ApiError>() {
        Ok(e.to_owned().into_response())
    } else if let Some(e) = err.find::<Error>() {
        Ok(ApiError::from(e).into_response())
    } else {
        Ok(ApiError::InternalError.into_response())
    }
}

fn rest(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    hub::hub(Arc::clone(&server))
        .or(channel::channel(Arc::clone(&server)))
        .or(member::member(Arc::clone(&server)))
        .or(message::message(Arc::clone(&server)))
}

fn auth() -> impl Filter<Extract = (ID,), Error = warp::Rejection> + Clone {
    warp::header("authorization")
}

fn with_server(
    server: ServerAddress,
) -> impl Filter<Extract = (ServerAddress,), Error = Infallible> + Clone {
    warp::any().map(move || Arc::clone(&server))
}

fn server_info() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("info")
        .map(move || SERVER_INFO_STRING.as_str().to_string())
        .and_then(|server_info: String| async move { Ok::<String, Rejection>(server_info) })
}

fn websocket(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("websocket")
        .and(with_server(server))
        .and(auth())
        .and(warp::ws())
        .and_then(handlers::websocket)
}

fn graphql(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    path!("graphql")
        .and(with_server(server))
        .and(auth())
        .and(async_graphql_warp::graphql(schema))
        .and_then(handlers::graphql)
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
                    GraphQLPlaygroundConfig::new("/api/graphql")
                        .with_header("authorization", user.to_string().as_str()),
                )),
        )
    })
}

mod hub {
    use super::*;
    use handlers::hub;

    fn create() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::path::end()
            .and(warp::post())
            .and(auth())
            .and(warp::body::json())
            .and_then(hub::create)
    }

    fn update(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(hub::update)
    }

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID).and(warp::get()).and(auth()).and_then(hub::get)
    }

    fn delete(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::delete())
            .and(auth())
            .and(with_server(server))
            .and_then(hub::delete)
    }

    fn join(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / "join")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(hub::join)
    }

    fn leave(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / "leave")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(hub::leave)
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
    use handlers::message;

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / ID)
            .and(warp::get())
            .and(auth())
            .and_then(message::get)
    }

    fn get_after() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "after")
            .and(warp::get())
            .and(warp::body::json())
            .and(auth())
            .and_then(message::get_after)
    }

    fn get_before() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "before")
            .and(warp::get())
            .and(warp::body::json())
            .and(auth())
            .and_then(message::get_before)
    }

    fn get_time_period() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "between")
            .and(warp::get())
            .and(warp::body::json())
            .and(auth())
            .and_then(message::get_between)
    }

    fn send(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::post())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(message::send)
    }

    pub fn message(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("message" / ..).and(
            send(Arc::clone(&server))
                .or(get_time_period())
                .or(get_after())
                .or(get_before())
                .or(get()),
        )
    }
}

mod channel {
    use super::*;
    use handlers::channel;

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::get())
            .and(auth())
            .and_then(channel::get)
    }

    fn create(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID)
            .and(warp::post())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(channel::create)
    }

    fn update(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json())
            .and(with_server(server))
            .and_then(channel::update)
    }

    fn delete(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID)
            .and(warp::delete())
            .and(auth())
            .and(with_server(server))
            .and_then(channel::delete)
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
    use super::*;
    use crate::permission::{ChannelPermission, HubPermission};
    use handlers::member;

    fn status() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::get()
            .and(auth())
            .and(path!(ID / ID / "status"))
            .and_then(member::status)
    }

    fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::get()
            .and(auth())
            .and(path!(ID / ID))
            .and_then(member::get)
    }

    fn kick(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "kick")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(member::kick)
    }

    fn mute(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "mute")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(member::mute)
    }

    fn ban(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "ban")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(member::ban)
    }

    fn unban(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "unban")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(member::unban)
    }

    fn unmute(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "unmute")
            .and(warp::post())
            .and(auth())
            .and(with_server(server))
            .and_then(member::unmute)
    }

    fn set_hub_permission(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "hub_permission" / HubPermission)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json().map(|s: HttpSetPermission| s.setting))
            .and(with_server(server))
            .and_then(member::set_hub_permission)
    }

    fn get_hub_permission() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "hub_permission" / HubPermission)
            .and(warp::get())
            .and(auth())
            .and_then(member::get_hub_permission)
    }

    fn set_channel_permission(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "channel_permission" / ID / ChannelPermission)
            .and(warp::put())
            .and(auth())
            .and(warp::body::json().map(|s: HttpSetPermission| s.setting))
            .and(with_server(server))
            .and_then(member::set_channel_permission)
    }

    fn get_channel_permission() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!(ID / ID / "channel_permission" / ID / ChannelPermission)
            .and(warp::get())
            .and(auth())
            .and_then(member::get_channel_permission)
    }

    pub fn member(
        server: ServerAddress,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        path!("member" / ..).and(
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
