use crate::config::Config;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::Response;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use warp::Reply;
use warp::{http::Response as HttpResponse, Filter};

use pgp::Deserializable;

use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use async_graphql::*;
use xactor::{Actor, Addr};

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let graphql_server_arc = server.clone();

    let graphql_post = warp::any()
        .map(move || (graphql_server_arc.clone(), schema.clone()))
        .and(warp::path("graphql"))
        .and(warp::body::bytes())
        .and_then(
            |(server, schema): (
                Arc<Addr<Server>>,
                Schema<QueryRoot, MutationRoot, EmptySubscription>,
            ),
             body: warp::hyper::body::Bytes| async move {
                Ok::<_, Infallible>(
                    async {
                        let body = body.to_vec();
                        let message = pgp::Message::from_bytes(&mut body.as_slice())?;
                        let content = message
                            .get_literal()
                            .ok_or(Error::InvalidMessage)?
                            .to_string()
                            .ok_or(Error::InvalidMessage)?;
                        let signature = message.clone().into_signature().signature;
                        let key_id = signature.issuer().ok_or(Error::InvalidMessage)?;
                        let pub_key = crate::signing::get_or_import_public_key(key_id)?;
                        message.verify(&pub_key)?;
                        let request = Request::new(content);
                        let resp = schema
                            .execute(request.data(server).data(hex::encode(key_id)))
                            .await;
                        Ok::<_, Error>(resp)
                    }
                    .await
                    .map_or_else(|e| e.into_response(), |o| Response::from(o).into_response()),
                )
            },
        );

    let web_socket =
        warp::path!("v2" / "websocket")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let server = server.clone();
                ws.on_upgrade(move |websocket| async move {
                    let _ =
                        crate::websocket::handle_connection(websocket, "test".to_string(), server)
                            .await;
                })
            });

    let graphql_playground = warp::path("graphql_playground").and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/"),
            ))
    });

    let routes = graphql_playground.or(graphql_post).or(web_socket);
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
