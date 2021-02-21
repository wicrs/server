use actix_web::http;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    wicrs_server::start("127.0.0.1:8080").await
}
