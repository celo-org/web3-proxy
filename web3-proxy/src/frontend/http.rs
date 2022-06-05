use crate::app::Web3ProxyApp;
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde_json::json;
use std::sync::Arc;

/// a page for configuring your wallet with all the rpcs
/// TODO: check auth (from authp?) here
/// TODO: return actual html
pub async fn index() -> impl IntoResponse {
    "Hello, World!"
}

/// Very basic status page
pub async fn status(app: Extension<Arc<Web3ProxyApp>>) -> impl IntoResponse {
    let app = app.0.as_ref();

    let balanced_rpcs = app.get_balanced_rpcs();
    let private_rpcs = app.get_private_rpcs();
    let num_active_requests = app.get_active_requests().len();

    // TODO: what else should we include? uptime? prometheus?
    let body = json!({
        "balanced_rpcs": balanced_rpcs,
        "private_rpcs": private_rpcs,
        "num_active_requests": num_active_requests,
    });

    (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
}
