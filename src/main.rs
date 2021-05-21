#[macro_use]
extern crate log;
extern crate pretty_env_logger;

use log::LevelFilter;

/// Main function, loads config and starts a server for the HTTP API.
#[tokio::main]
async fn main() {
    let mut builder = pretty_env_logger::formatted_timed_builder();
    builder.filter_level(LevelFilter::Info);
    builder.parse_filters("RUST_LOG");
    builder.init();

    if let Err(err) = wicrs_server::start().await {
        error!("{}", err);
    } else {
        info!("WICRS Server stopped.")
    }
}
