use utoipa_axum::{router::OpenApiRouter, routes};

use crate::state::AppState;

mod connections;
mod runs;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(connections::providers))
        .routes(routes!(connections::list, connections::create))
        .routes(routes!(connections::update, connections::delete))
        .routes(routes!(connections::test))
        .routes(routes!(connections::preview))
        .routes(routes!(runs::sync))
        .routes(routes!(runs::list))
        .routes(routes!(runs::get))
}
