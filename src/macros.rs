macro_rules! api_get {
    (($name:ident, $($datatype:ty)?, $($path:expr)?) [$account:ident, $query:ident] $($do:tt)*) => {
        fn $name(
            auth_manager: std::sync::Arc<tokio::sync::Mutex<crate::auth::Auth>>
        ) -> warp::filters::BoxedFilter<(impl warp::Reply,)> {
            use reqwest::StatusCode;
            use warp::Filter;
            use warp::Reply;
            #[derive(Deserialize)]
            struct AccountToken {
                account: String,
                token: String,
            }
            warp::get()
                $(.and($path))?
                .and(warp::query::<AccountToken>())
                $(.and(warp::body::json::<$datatype>()))?
                .and_then(move |auth_query: AccountToken$(, $query: $datatype)?| {
                    let tmp_auth = auth_manager.clone();
                    async move {
                        Ok::<warp::http::Response<warp::hyper::Body>, warp::Rejection>(
                            if crate::auth::Auth::is_authenticated(
                                tmp_auth,
                                &auth_query.account,
                                auth_query.token,
                            )
                            .await
                            {
                                if let Ok($account) = crate::user::Account::load(&auth_query.account).await {
                                    $($do)*
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
