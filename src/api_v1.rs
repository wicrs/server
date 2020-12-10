use crate::user::User;
use reqwest::StatusCode;
use serde::Deserialize;
use warp::Reply;

use crate::NAME_ALLOWED_CHARS;

fn unexpected_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Unexpected error.", StatusCode::INTERNAL_SERVER_ERROR).into_response()
}

fn bad_auth_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Invalid authentication details.", StatusCode::FORBIDDEN)
        .into_response()
}

fn account_not_found_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Could not find that account.", StatusCode::NOT_FOUND).into_response()
}

#[derive(Deserialize)]
struct AccountToken {
    account: String,
    token: String
}

macro_rules! api_get {
    ($name:ident, $path:expr, $datatype:ty, $account:ident, $query:ident, $do:block) => {
        fn $name(
            auth_manager: std::sync::Arc<tokio::sync::Mutex<crate::auth::Auth>>,
        ) -> warp::filters::BoxedFilter<(impl warp::Reply,)> {
            use crate::auth::Auth;
            use crate::user::Account;
            use reqwest::StatusCode;
            use warp::Filter;
            use warp::Reply;
            warp::get()
                .and($path)
                .and(warp::query::<AccountToken>())
                .and(warp::body::json::<$datatype>())
                .and_then(move |auth_query: AccountToken, $query: $datatype| {
                    let tmp_auth = auth_manager.clone();
                    async move {
                        Ok::<warp::http::Response<warp::hyper::Body>, warp::Rejection>(
                            if Auth::is_authenticated(
                                tmp_auth,
                                &auth_query.account,
                                auth_query.token,
                            )
                            .await
                            {
                                if let Ok(mut $account) = Account::load(&auth_query.account).await {
                                    $do
                                } else {
                                    warp::reply::with_status(
                                        "Could not find that account.",
                                        StatusCode::NOT_FOUND,
                                    )
                                    .into_response()
                                }
                            } else {
                                warp::reply::with_status(
                                    "Invalid authentication details.",
                                    StatusCode::FORBIDDEN,
                                )
                                .into_response()
                            },
                        )
                    }
                })
                .boxed()
        }
    };
}

api_get!(
    get_account,
    warp::path("account"),
    AccountToken,
    account,
    query,
    { warp::reply::json(&account).into_response() }
);

#[derive(Deserialize)]
struct AddUserQuery {
    id: String,
    token: String,
    username: String,
}

api_get!(
    add_user,
    warp::path("account").and(warp::path("adduser")),
    AddUserQuery,
    account,
    query,
    {
        use crate::ApiActionError;
        let create: Result<User, ApiActionError> = account.create_new_user(query.username).await;
        if let Ok(user) = create {
            warp::reply::json(&user).into_response()
        } else {
            match create.err() {
                Some(ApiActionError::WriteFileError) => warp::reply::with_status(
                    "Server could not write user data to disk.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response(),
                Some(ApiActionError::BadNameCharacters) => warp::reply::with_status(
                    format!(
                        "Username string can only contain the following characters: \"{}\"",
                        NAME_ALLOWED_CHARS
                    ),
                    StatusCode::BAD_REQUEST,
                )
                .into_response(),
                _ => unexpected_response(),
            }
        }
    }
);
