use serde::{Deserialize, Serialize};

/// Configuration object for WICRS Server.
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Base URL of the PGP key server to use.
    pub key_server: String,
    /// Address to listen on for HTTP requests. (`host:port`)
    pub address: String,
    /// Whether or not to show the version of WICRS server on the root webpage (`http(s)://host:port/`)
    pub show_version: bool,
}
