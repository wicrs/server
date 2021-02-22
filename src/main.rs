#[actix_web::main]
async fn main() -> std::io::Result<()> {
    wicrs_server::start(&wicrs_server::CONFIG.address).await
}
