use crate::{
    user::{GenericUser, User},
};

use actix_web::{get, post, web::{self, Path, Json}, App, HttpServer, Responder, http::StatusCode};

pub(crate) async fn server(bind_address: &str) -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index).service(get_user).service(get_user_by_id))
        .bind(bind_address)?
        .run()
        .await
}

#[get("/")]
async fn index() -> impl Responder {
    "WICRS is up and running!"
}

#[get("/user")]
async fn get_user(user: User) -> impl Responder {
    Json(user)
}

#[get("/user/{id}")]
async fn get_user_by_id(_user: User, id: Path<String>) -> actix_web::Result<impl Responder> {
    if let Ok(other) = User::load(id.0.as_str()).await {
        Ok(Json(other.to_generic()))
    } else {
        Err(actix_web::error::ErrorNotFound("User not found."))
    }
}
