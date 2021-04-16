use serde::{Deserialize, Serialize};

/// Configuration object for WICRS Server.
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Authentication services
    pub auth_services: AuthConfigs,
    /// Address to listen on for HTTP requests. (`host:port`)
    pub address: String,
    /// Whether or not to show the version of WICRS server on the root webpage (`http(s)://host:port/`)
    pub show_version: bool,
    /// How often to send heartbeat pings (in milliseconds)
    pub ws_hb_interval: u64,
    /// How long before lack of client response causes a timeout (in milliseconds)
    pub ws_client_timeout: u64
}

/// Configuration for a generic OAuth service.
#[derive(Serialize, Deserialize, Clone)]
pub struct AuthConfig {
    /// Whether or not this OAuth service should be used.
    pub enabled: bool,
    /// Client ID given by the OAuth service.
    pub client_id: String,
    /// Client Secret given by the OAuth service.
    pub client_secret: String,
}

/// OAuth service configurations.
#[derive(Serialize, Deserialize, Clone)]
pub struct AuthConfigs {
    /// GitHub OAuth config.
    pub github: Option<AuthConfig>,
}
