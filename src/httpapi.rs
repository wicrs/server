use crate::{
    auth::{Auth, AuthQuery, Service},
    config::Config,
};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::Response;
use reqwest::StatusCode;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{http::Response as HttpResponse, Filter, Rejection};

use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use crate::ID;
use async_graphql::Response as AsyncGraphQLResponse;
use async_graphql::*;
use xactor::{Actor, Addr};

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let auth = Auth::from_config(&config.auth_services);
    let auth = Arc::new(RwLock::new(auth));

    let auth = warp::any().map(move || auth.clone());
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let graphql_server_arc = server.clone();

    let graphql_post = warp::any()
        .map(move || graphql_server_arc.clone())
        .and(warp::path("graphql"))
        .and(warp::header::<String>("authorization"))
        .and(async_graphql_warp::graphql(schema.clone()))
        .and(auth.clone())
        .and_then(
            |server: Arc<Addr<Server>>,
             token: String,
             (schema, request): (
                Schema<QueryRoot, MutationRoot, EmptySubscription>,
                async_graphql::Request,
            ),
             auth: Arc<RwLock<Auth>>| async move {
                let mut split = token.as_str().split(':');
                if let (Some(id), Some(token)) = (split.next(), split.next()) {
                    if let Ok(id) = ID::parse_str(id) {
                        if Auth::is_authenticated(auth.clone(), id.clone(), token.into()).await {
                            let resp = schema.execute(request.data(id).data(server)).await;
                            return Ok::<_, Infallible>(Response::from(resp));
                        }
                    }
                }
                Ok::<_, Infallible>(Response::from(AsyncGraphQLResponse::new(
                    "Not Authenticated.",
                )))
            },
        );

    let auth_filter = warp::header::<String>("authorization")
        .and(auth.clone())
        .and_then(|id_token: String, auth: Arc<RwLock<Auth>>| async move {
            let mut split = id_token.as_str().split(':');
            if let (Some(id), Some(token)) = (split.next(), split.next()) {
                if let Ok(id) = ID::parse_str(id) {
                    if Auth::is_authenticated(auth.clone(), id.clone(), token.into()).await {
                        return Ok((id, auth));
                    }
                }
            }
            return Err(warp::reject::custom(Error::NotAuthenticated));
        });

    let auth_start = warp::path!("v2" / "login" / Service)
        .map(move |service| service)
        .and(auth.clone())
        .and_then(|service, auth| async move {
            let redirect = Auth::start_login(auth, service).await;
            Ok::<_, Infallible>(warp::reply::with_status(
                warp::reply::with_header(warp::reply(), "Location", &redirect),
                StatusCode::FOUND,
            ))
        });

    let auth_finish = warp::path!("v2" / "auth" / Service)
        .and(warp::query::<AuthQuery>())
        .and(auth.clone())
        .and_then(|service, query, auth| async move {
            Ok::<_, Rejection>(warp::reply::json(
                &Auth::handle_oauth(auth, service, query)
                    .await
                    .map_err(|err| Rejection::from(err))?,
            ))
        });

    let invalidate_token = warp::path!("v2" / "invalidate_token" / String)
        .and(auth_filter.clone())
        .and_then(|token, (id, auth)| async move {
            Auth::invalidate_token(auth, id, token).await;
            Ok::<_, Infallible>(warp::reply::with_status(
                warp::reply(),
                StatusCode::NO_CONTENT,
            ))
        });

    let invalidate_tokens = warp::path!("v2" / "invalidate_tokens")
        .and(auth_filter.clone())
        .and_then(|(id, auth)| async move {
            Auth::invalidate_all_tokens(auth, id).await;
            Ok::<_, Infallible>(warp::reply::with_status(
                warp::reply(),
                StatusCode::NO_CONTENT,
            ))
        });

    let graphql_playground = warp::path!("graphql_playground" / String)
        .and(warp::get())
        .map(move |auth_string: String| {
            HttpResponse::builder()
                .header("content-type", "text/html")
                .body(playground_source(
                    GraphQLPlaygroundConfig::new(format!("/graphql").as_str())
                        .with_header("authorization", &auth_string)
                        .subscription_endpoint("/"),
                ))
        });

    let routes = graphql_playground
        .or(graphql_post)
        .or(auth_start)
        .or(auth_finish)
        .or(invalidate_token)
        .or(invalidate_tokens);
    warp::serve(routes)
        .run(
            config
                .address
                .parse::<SocketAddr>()
                .expect("Unable to parse server bind address."),
        )
        .await;
    Ok(())
}
