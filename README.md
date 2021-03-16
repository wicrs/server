# WICRS Server

[![crates.io](https://img.shields.io/crates/v/wicrs_server.svg)](https://crates.io/crates/wicrs_server)
[![docs.rs](https://docs.rs/wicrs_server/badge.svg)](https://docs.rs/wicrs_server)

A server for handling chat rooms and messaging groups written in rust.

## Build

Install Rust by following [these](https://www.rust-lang.org/tools/install) instructions.
Then clone the git repo, then to build:

```bash
git clone https://github.com/wicrs/server.git wicrs_server
cd wicrs_server
cargo build # to build the release version run cargo build --release
```

## Setup

First you need to create a GitHub OAuth application by following the instructions [here](https://docs.github.com/en/free-pro-team@latest/developers/apps/creating-an-oauth-app), make sure to set the callback URL to `$HOSTNAME:$PORT/api/v2/auth/github`, replace `$PORT` with the port you choose in the config and replace `$HOSTNAME` with the address you will navigate to when accessing the WICRS API.

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
    "address": "127.0.0.1:8080",
    "show_version": true
}
```

Make sure to replace `$GITHUB_CLIENT_ID` with the client ID and `$GITHUB_CLIENT_SECRET` with the client secret you got when you created the GitHub OAuth application.
`address` should be set to the local address you want the server to listen on, for example you can use `127.0.0.1:8080`. The `show_version` variable determines whether or not the server will tell clients it's version when they go to the HTTP root (`/`).

Note that the server application needs to be able to read `./config.json` and must be able to read and write to `./data` or most if not all requests will fail.

Once this is done run the server by executing `cargo run` or `cargo run --release` if you are in the project git directory. If you are not in the project's git directory you will need to either put the executable in the desired run directory (where you have the `config.json` file) and run `./wicrs_server`. Otherwise you need to have it in your path in which case you just need to run `wicrs_server` in your chosen run directory.

## Developing and Contributing

To contribute fork the GitHub repo and make your changes, for changes to be accepted your fork must pass all of the tests, to run the tests go to the root directory of the project and run `cargo test`. If you add any features make sure to add tests to ensure they work.
