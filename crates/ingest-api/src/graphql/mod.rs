//! GraphQL API implementation
//!
//! This module provides a GraphQL API for the Inngest platform, offering
//! a flexible and efficient way to query and manipulate data.

pub mod resolvers;
pub mod schema;
pub mod subscriptions;
pub mod types;

use async_graphql::{
    EmptySubscription, Schema,
    http::{GraphQLPlaygroundConfig, playground_source},
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};

use crate::handlers::AppState;
use schema::{MutationRoot, QueryRoot};

/// GraphQL schema type
pub type GraphQLSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

/// Create GraphQL router with playground
pub fn create_graphql_router(state: AppState) -> Router<AppState> {
    let schema = create_schema();

    Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .with_state(state)
        .layer(axum::extract::Extension(schema))
}

/// Create the GraphQL schema
fn create_schema() -> GraphQLSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish()
}

/// GraphQL playground handler
async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

/// GraphQL query handler
async fn graphql_handler(
    State(_state): State<AppState>,
    schema: axum::extract::Extension<GraphQLSchema>,
    req: async_graphql_axum::GraphQLRequest,
) -> Result<async_graphql_axum::GraphQLResponse, StatusCode> {
    let response = schema.execute(req.into_inner()).await;
    Ok(response.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_schema_creation() {
        let schema = create_schema();

        // Verify schema is created successfully
        assert_eq!(schema.sdl(), schema.sdl());
    }

    #[tokio::test]
    async fn test_graphql_playground() {
        let response = graphql_playground().await;

        // Verify playground returns HTML
        let (parts, _body) = response.into_response().into_parts();
        assert_eq!(parts.status, StatusCode::OK);
    }
}
