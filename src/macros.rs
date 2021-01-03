macro_rules! api_get {
    (($name:ident, $($datatype:ty)?, $($path:expr)?) [$auth:ident, $account:ident, $query:ident] $($do:tt)*) => {
        fn $name(
            auth_manager: std::sync::Arc<tokio::sync::Mutex<crate::auth::Auth>>
        ) -> warp::filters::BoxedFilter<(impl warp::Reply,)> {
            use reqwest::StatusCode;
            use warp::Filter;
            use warp::Reply;
            #[derive(Deserialize)]
            struct UserToken {
                user: String,
                token: String,
            }
            warp::get()
                $(.and($path))?
                .and(warp::query::<UserToken>())
                $(.and(warp::body::json::<$datatype>()))?
                .and_then(move |auth_query: UserToken$(, $query: $datatype)?| {
                    let $auth = auth_manager.clone();
                    async move {
                        Ok::<warp::http::Response<warp::hyper::Body>, warp::Rejection>(
                            if crate::auth::Auth::is_authenticated(
                                $auth.clone(),
                                &auth_query.user,
                                auth_query.token,
                            )
                            .await
                            {
                                if let Ok($account) = crate::user::User::load(&auth_query.user).await {
                                    $($do)*
                                } else {
                                    warp::reply::with_status(
                                        "Server failed to load the user.",
                                        StatusCode::INTERNAL_SERVER_ERROR,
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
