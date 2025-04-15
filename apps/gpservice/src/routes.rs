use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{handlers, ws_server::WsServerContext};

pub(crate) fn routes(ctx: Arc<WsServerContext>) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/ws", get(handlers::ws_handler))
        .with_state(ctx)
}
