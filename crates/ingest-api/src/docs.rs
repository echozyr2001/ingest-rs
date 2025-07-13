//! API documentation generation and serving
//!
//! This module provides OpenAPI/Swagger documentation generation and
//! interactive documentation serving capabilities.

use axum::{
    Json, Router,
    extract::Path,
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use serde_json::Value;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

use crate::handlers::AppState;

/// OpenAPI documentation structure
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Inngest API",
        version = "1.0.0",
        description = "REST API for the Inngest durable functions platform"
    ),
    components(
        schemas(
            EventRequest,
            EventResponse,
            FunctionResponse,
            FunctionListResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "events", description = "Event management endpoints"),
        (name = "functions", description = "Function management endpoints"),
        (name = "executions", description = "Function execution endpoints"),
    )
)]
pub struct ApiDoc;

/// Security scheme modifier
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
            );
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
            );
        }
    }
}

/// Event request schema for documentation
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct EventRequest {
    /// Event name/type
    #[schema(example = "user.created")]
    pub name: String,
    /// Event data payload
    #[schema(example = json!({"user_id": "123", "email": "user@example.com"}))]
    pub data: serde_json::Value,
    /// User identifier
    #[schema(example = "user123")]
    pub user: Option<String>,
    /// Event version
    #[schema(example = "1.0")]
    pub version: Option<String>,
}

/// Event response schema for documentation
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct EventResponse {
    /// Event ID
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: String,
    /// Event name/type
    #[schema(example = "user.created")]
    pub name: String,
    /// Event data payload
    #[schema(example = json!({"user_id": "123", "email": "user@example.com"}))]
    pub data: serde_json::Value,
    /// User identifier
    #[schema(example = "user123")]
    pub user: Option<String>,
    /// Event timestamp
    #[schema(example = "2023-01-01T00:00:00Z")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event version
    #[schema(example = "1.0")]
    pub version: Option<String>,
}

/// Function response schema for documentation
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct FunctionResponse {
    /// Function ID
    #[schema(example = "456e7890-e89b-12d3-a456-426614174000")]
    pub id: String,
    /// Function name
    #[schema(example = "process-user")]
    pub name: String,
    /// Function description
    #[schema(example = "Process user creation events")]
    pub description: Option<String>,
    /// Function status
    #[schema(example = "active")]
    pub status: String,
    /// Function configuration
    pub config: FunctionConfigResponse,
    /// Function triggers
    pub triggers: Vec<TriggerResponse>,
    /// Creation timestamp
    #[schema(example = "2023-01-01T00:00:00Z")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    #[schema(example = "2023-01-01T00:00:00Z")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Function configuration response schema
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct FunctionConfigResponse {
    /// Function timeout in seconds
    #[schema(example = 30)]
    pub timeout: u32,
    /// Maximum retries
    #[schema(example = 3)]
    pub retries: u32,
    /// Concurrency limit
    #[schema(example = 10)]
    pub concurrency: Option<u32>,
}

/// Trigger response schema
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct TriggerResponse {
    /// Trigger type
    #[schema(example = "event")]
    pub trigger_type: String,
    /// Event name for event triggers
    #[schema(example = "user.created")]
    pub event: Option<String>,
    /// Cron expression for scheduled triggers
    #[schema(example = "0 0 * * *")]
    pub cron: Option<String>,
}

/// Function list response schema
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct FunctionListResponse {
    /// List of functions
    pub functions: Vec<FunctionResponse>,
    /// Pagination metadata
    pub pagination: PaginationResponse,
}

/// Pagination response schema
#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct PaginationResponse {
    /// Current page number
    #[schema(example = 1)]
    pub page: u32,
    /// Number of items per page
    #[schema(example = 10)]
    pub per_page: u32,
    /// Total number of items
    #[schema(example = 100)]
    pub total: u64,
    /// Total number of pages
    #[schema(example = 10)]
    pub total_pages: u32,
    /// Whether there are more pages
    #[schema(example = true)]
    pub has_next: bool,
    /// Whether there are previous pages
    #[schema(example = false)]
    pub has_prev: bool,
}

/// Create documentation router
pub fn create_docs_router() -> Router<AppState> {
    Router::new()
        .route("/docs/{*path}", get(serve_swagger_ui))
        .route("/api-docs/openapi.json", get(serve_openapi))
        .route("/docs", get(serve_docs_redirect))
        .route("/redoc", get(serve_redoc))
}

/// Serve Swagger UI
async fn serve_swagger_ui(Path(path): Path<String>) -> impl IntoResponse {
    match path.as_str() {
        "" | "/" => {
            // Redirect to the main Swagger UI page
            Redirect::permanent("/docs/index.html").into_response()
        }
        _ => {
            // Serve Swagger UI assets
            Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@3.52.5/swagger-ui.css" />
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@3.52.5/swagger-ui-bundle.js"></script>
    <script>
        SwaggerUIBundle({
            url: '/api-docs/openapi.json',
            dom_id: '#swagger-ui',
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIBundle.presets.standalone
            ]
        });
    </script>
</body>
</html>
            "#).into_response()
        }
    }
}
/// Serve OpenAPI specification
async fn serve_openapi() -> Json<Value> {
    let openapi = ApiDoc::openapi();
    let json_str = openapi
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI spec");
    let json_value: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");
    Json(json_value)
}

/// Redirect to Swagger UI
async fn serve_docs_redirect() -> impl IntoResponse {
    axum::response::Redirect::permanent("/docs/")
}

/// Serve ReDoc documentation
async fn serve_redoc() -> Html<String> {
    let html = r#"
<!DOCTYPE html>
<html>
  <head>
    <title>Inngest API Documentation</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link href="https://fonts.googleapis.com/css?family=Montserrat:300,400,700|Roboto:300,400,700" rel="stylesheet">
    <style>
      body {
        margin: 0;
        padding: 0;
      }
    </style>
  </head>
  <body>
    <redoc spec-url='/api-docs/openapi.json'></redoc>
    <script src="https://cdn.jsdelivr.net/npm/redoc@2.0.0/bundles/redoc.standalone.js"> </script>
  </body>
</html>
"#;
    Html(html.to_string())
}

/// Generate API examples for documentation
pub fn generate_api_examples() -> serde_json::Value {
    serde_json::json!({
        "event_creation": {
            "description": "Create a new event",
            "request": {
                "method": "POST",
                "url": "/events",
                "headers": {
                    "Content-Type": "application/json",
                    "Authorization": "Bearer your-jwt-token"
                },
                "body": {
                    "name": "user.created",
                    "data": {
                        "user_id": "123",
                        "email": "user@example.com",
                        "name": "John Doe"
                    },
                    "user": "user123",
                    "version": "1.0"
                }
            },
            "response": {
                "status": 201,
                "body": {
                    "id": "123e4567-e89b-12d3-a456-426614174000",
                    "name": "user.created",
                    "data": {
                        "user_id": "123",
                        "email": "user@example.com",
                        "name": "John Doe"
                    },
                    "user": "user123",
                    "timestamp": "2023-01-01T00:00:00Z",
                    "version": "1.0"
                }
            }
        },
        "function_listing": {
            "description": "List all functions",
            "request": {
                "method": "GET",
                "url": "/functions?page=1&per_page=10",
                "headers": {
                    "Authorization": "Bearer your-jwt-token"
                }
            },
            "response": {
                "status": 200,
                "body": {
                    "functions": [
                        {
                            "id": "456e7890-e89b-12d3-a456-426614174000",
                            "name": "process-user",
                            "description": "Process user creation events",
                            "status": "active",
                            "config": {
                                "timeout": 30,
                                "retries": 3,
                                "concurrency": 10
                            },
                            "triggers": [
                                {
                                    "trigger_type": "event",
                                    "event": "user.created"
                                }
                            ],
                            "created_at": "2023-01-01T00:00:00Z",
                            "updated_at": "2023-01-01T00:00:00Z"
                        }
                    ],
                    "pagination": {
                        "page": 1,
                        "per_page": 10,
                        "total": 1,
                        "total_pages": 1,
                        "has_next": false,
                        "has_prev": false
                    }
                }
            }
        },
        "graphql_query": {
            "description": "GraphQL query example",
            "request": {
                "method": "POST",
                "url": "/graphql",
                "headers": {
                    "Content-Type": "application/json",
                    "Authorization": "Bearer your-jwt-token"
                },
                "body": {
                    "query": "query GetEvents($first: Int) { events(first: $first) { edges { node { id name timestamp } } pageInfo { hasNextPage } } }",
                    "variables": {
                        "first": 10
                    }
                }
            },
            "response": {
                "status": 200,
                "body": {
                    "data": {
                        "events": {
                            "edges": [
                                {
                                    "node": {
                                        "id": "123e4567-e89b-12d3-a456-426614174000",
                                        "name": "user.created",
                                        "timestamp": "2023-01-01T00:00:00Z"
                                    }
                                }
                            ],
                            "pageInfo": {
                                "hasNextPage": false
                            }
                        }
                    }
                }
            }
        }
    })
}

/// API usage guide content
pub fn generate_usage_guide() -> String {
    r#"
# Inngest API Usage Guide

## Authentication

The Inngest API supports multiple authentication methods:

### JWT Bearer Token
```bash
curl -H "Authorization: Bearer your-jwt-token" \
     https://api.inngest.com/events
```

### API Key
```bash
curl -H "X-API-Key: your-api-key" \
     https://api.inngest.com/events
```

## Rate Limiting

The API implements rate limiting to ensure fair usage:

- **Anonymous requests**: 60 requests per minute
- **Authenticated requests**: 1000 requests per minute
- **Burst allowance**: 10 requests

Rate limit headers are included in responses:
- `X-RateLimit-Limit`: Maximum requests per window
- `X-RateLimit-Remaining`: Remaining requests in current window
- `X-RateLimit-Reset`: Time until rate limit resets

## Error Handling

The API returns structured error responses:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid event data",
    "details": {
      "field": "name",
      "issue": "Event name is required"
    }
  }
}
```

Common HTTP status codes:
- `400 Bad Request`: Invalid request data
- `401 Unauthorized`: Missing or invalid authentication
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `429 Too Many Requests`: Rate limit exceeded
- `500 Internal Server Error`: Server error

## Pagination

List endpoints support cursor-based pagination:

```bash
curl "https://api.inngest.com/events?first=10&after=cursor_123"
```

Response includes pagination metadata:
```json
{
  "data": [...],
  "pageInfo": {
    "hasNextPage": true,
    "hasPreviousPage": false,
    "startCursor": "cursor_1",
    "endCursor": "cursor_10"
  },
  "totalCount": 100
}
```

## GraphQL API

The GraphQL endpoint is available at `/graphql` with an interactive playground at `/graphql` (GET request).

### Example Query
```graphql
query GetEvents($first: Int!) {
  events(first: $first) {
    edges {
      node {
        id
        name
        data
        timestamp
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
```

### Example Mutation
```graphql
mutation CreateEvent($input: CreateEventInput!) {
  createEvent(input: $input) {
    id
    name
    timestamp
  }
}
```

### Subscriptions
Real-time subscriptions are available for live updates:

```graphql
subscription {
  events {
    id
    name
    timestamp
  }
}
```

## SDK Usage

### JavaScript/TypeScript
```javascript
import { Inngest } from 'inngest';

const inngest = new Inngest({
  apiBaseUrl: 'https://api.inngest.com',
  apiKey: 'your-api-key'
});

// Send an event
await inngest.send({
  name: 'user.created',
  data: { userId: '123', email: 'user@example.com' }
});
```

### Python
```python
from inngest import Inngest

inngest = Inngest(
    api_base_url='https://api.inngest.com',
    api_key='your-api-key'
)

# Send an event
inngest.send({
    'name': 'user.created',
    'data': {'user_id': '123', 'email': 'user@example.com'}
})
```

## Best Practices

1. **Use appropriate authentication**: JWT tokens for user-specific operations, API keys for service-to-service communication
2. **Handle rate limits gracefully**: Implement exponential backoff for 429 responses
3. **Use pagination**: Don't fetch all data at once for large datasets
4. **Validate input**: Ensure event data conforms to expected schema
5. **Monitor usage**: Track API usage and performance metrics
6. **Use GraphQL for complex queries**: Reduce over-fetching with precise field selection

## Support

For additional support:
- Documentation: https://www.inngest.com/docs
- Community: https://discord.gg/inngest
- Support: support@inngest.com
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_openapi_generation() {
        let openapi = ApiDoc::openapi();

        assert_eq!(openapi.info.title, "Inngest API");
        assert_eq!(openapi.info.version, "1.0.0");
        assert!(openapi.info.description.is_some());
    }

    #[test]
    fn test_api_examples_generation() {
        let examples = generate_api_examples();

        assert!(examples["event_creation"].is_object());
        assert!(examples["function_listing"].is_object());
        assert!(examples["graphql_query"].is_object());
    }

    #[test]
    fn test_usage_guide_generation() {
        let guide = generate_usage_guide();

        assert!(guide.contains("Authentication"));
        assert!(guide.contains("Rate Limiting"));
        assert!(guide.contains("GraphQL API"));
        assert!(guide.contains("Best Practices"));
    }

    #[tokio::test]
    async fn test_serve_openapi() {
        let response = serve_openapi().await;
        let json_value = response.0;

        assert!(json_value["info"]["title"].as_str().unwrap() == "Inngest API");
        assert!(json_value["paths"].is_object());
        assert!(json_value["components"].is_object());
    }

    #[test]
    fn test_event_request_schema() {
        let event_request = EventRequest {
            name: "test.event".to_string(),
            data: serde_json::json!({"key": "value"}),
            user: Some("user123".to_string()),
            version: Some("1.0".to_string()),
        };

        let json = serde_json::to_string(&event_request).unwrap();
        assert!(json.contains("test.event"));
        assert!(json.contains("user123"));
    }

    #[test]
    fn test_function_response_schema() {
        let function_response = FunctionResponse {
            id: "123".to_string(),
            name: "test-function".to_string(),
            description: Some("Test function".to_string()),
            status: "active".to_string(),
            config: FunctionConfigResponse {
                timeout: 30,
                retries: 3,
                concurrency: Some(10),
            },
            triggers: vec![TriggerResponse {
                trigger_type: "event".to_string(),
                event: Some("test.event".to_string()),
                cron: None,
            }],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&function_response).unwrap();
        assert!(json.contains("test-function"));
        assert!(json.contains("active"));
    }
}
