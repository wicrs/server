# WICRS Server

[![crates.io](https://img.shields.io/crates/v/wicrs_server.svg)](https://crates.io/crates/wicrs_server)
[![docs.rs](https://docs.rs/wicrs_server/badge.svg)](https://docs.rs/wicrs_server)

A server for handling chat rooms and messaging groups written in rust.

## Build

Install Rust by following [these](https://www.rust-lang.org/tools/install) instructions.
Then clone the git repo and build:

```bash
git clone https://github.com/wicrs/server.git wicrs_server
cd wicrs_server
cargo build # to build the release version run cargo build --release
```

## Setup

First you need to create a GitHub OAuth application by following the instructions [here](https://docs.github.com/en/free-pro-team@latest/developers/apps/creating-an-oauth-app), make sure to set the callback URL to `$HOSTNAME:$PORT/api/v1/auth/github`, replace `$PORT` with the port you choose in the config and replace `$HOSTNAME` with the address you will navigate to when accessing the WICRS API.

To run the server you first need to create a config file named `config.json` in the server's working directory, which should be reserved for the server.
Here is an example of what the contents of `config.json` should be:

```json
{
    "auth_services": {
        "github": {
            "enabled": true,
            "client_id": "$GITHUB_CLIENT_ID",
            "client_secret": "$GITHUB_CLIENT_SECRET"
        }
    },
    "listen": [0, 0, 0, 0],
    "port": 24816
}
```

Make sure to replace `$GITHUB_CLIENT_ID` with the client ID and `$GITHUB_CLIENT_SECRET` with the client secret you got when you created the GitHub OAuth application.
`listen` should be set to the local IP address youw ant the server to listen on, for localhost use `[127, 0, 0, 1]`.
You can also set the GitHub client ID and secret with environement variables (which will be used instead of any configuration values) the variables are `$GITHUB_CLIENT_ID` and `$GITHUB_CLIENT_SECRET`.

## Developing and Contributing

To contribute fork the GitHub repo and make your changes, for changes to be accepted your fork must pass all of the tests, to run the tests go to the root directory of the project and run `cargo test`. If you add any features make sure to add tests to ensure they work.
