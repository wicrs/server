use std::{sync::Arc, collections::HashMap};
use tokio::sync::Mutex;
use warp::{
    http::{Response, StatusCode},
    Filter,
};



pub async fn local_oauth(logins: Arc<Mutex<Vec<()>>>) {
    let login = warp::get().and(warp::path("login"))
    let authenticate = warp::get()
        .and(warp::path("authenticate"))
        .and(warp::query::<HashMap<String, String>>())
        .map(|p: HashMap<String, String>| match p.get("code") {
            Some(code) => {
                match p.get("state") {
                    Some(state) => {
                        println!("state: {}, code: {}", state, code);
                        Response::builder().status(StatusCode::OK).body("")
                    }
                    None => Response::builder().status(StatusCode::BAD_REQUEST).body("")
                }
            }
            None => Response::builder().status(StatusCode::BAD_REQUEST).body("")
        });
    warp::serve(authenticate).run(([127, 0, 0, 1], 24816)).await;
}
