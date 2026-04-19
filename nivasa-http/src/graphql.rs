use crate::{Body, Json, NivasaRequest, NivasaResponse, NivasaServerBuilder};
use async_graphql::{ObjectType, SubscriptionType};
use http::StatusCode;
pub use nivasa_graphql::{
    EmptyMutation, EmptySubscription, GraphQLModule as GraphQLCoreModule,
    GraphQLRequest as GraphQLCoreRequest, GraphQLResponse as GraphQLCoreResponse, GraphQLSchema,
};
use nivasa_routing::{RouteDispatchError, RouteMethod};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::future::Future;
use std::sync::Arc;

/// GraphQL HTTP request envelope.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(default)]
    pub operation_name: Option<String>,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default)]
    pub extensions: Option<Value>,
}

/// GraphQL execution error payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

impl GraphQLError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            extensions: None,
        }
    }
}

/// GraphQL HTTP response envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphQLResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<GraphQLError>,
}

impl GraphQLResponse {
    pub fn data(data: impl Into<Value>) -> Self {
        Self {
            data: Some(data.into()),
            errors: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            data: None,
            errors: vec![GraphQLError::new(message)],
        }
    }

    fn into_json(self) -> Value {
        serde_json::to_value(self).expect("GraphQL response must serialize")
    }
}

type GraphQLExecutor = Arc<dyn Fn(GraphQLRequest) -> GraphQLResponse + Send + Sync + 'static>;

/// Minimal GraphQL HTTP wrapper with a POST endpoint and a playground page.
///
/// The module keeps the transport surface small: users provide an executor
/// closure, and the HTTP layer handles request envelope parsing plus the
/// interactive playground page.
pub struct GraphQLModule {
    endpoint_path: String,
    playground_path: String,
    title: String,
    executor: GraphQLExecutor,
}

impl GraphQLModule {
    pub fn new(
        executor: impl Fn(GraphQLRequest) -> GraphQLResponse + Send + Sync + 'static,
    ) -> Self {
        Self {
            endpoint_path: "/graphql".to_string(),
            playground_path: "/graphql".to_string(),
            title: "Nivasa GraphQL".to_string(),
            executor: Arc::new(executor),
        }
    }

    /// Build a GraphQL HTTP wrapper from a real async-graphql schema.
    pub fn from_schema<Q, M, S>(schema: GraphQLSchema<Q, M, S>) -> Self
    where
        Q: ObjectType + Send + Sync + 'static,
        M: ObjectType + Send + Sync + 'static,
        S: SubscriptionType + Send + Sync + 'static,
    {
        Self::from_graphql_module(GraphQLCoreModule::from_schema(schema))
    }

    /// Build a GraphQL HTTP wrapper from a real async-graphql module.
    pub fn from_graphql_module<Q, M, S>(module: GraphQLCoreModule<Q, M, S>) -> Self
    where
        Q: ObjectType + Send + Sync + 'static,
        M: ObjectType + Send + Sync + 'static,
        S: SubscriptionType + Send + Sync + 'static,
    {
        Self {
            endpoint_path: "/graphql".to_string(),
            playground_path: "/graphql".to_string(),
            title: "Nivasa GraphQL".to_string(),
            executor: core_graphql_executor(module),
        }
    }

    pub fn endpoint_path(mut self, path: impl Into<String>) -> Self {
        self.endpoint_path = path.into();
        self
    }

    pub fn playground_path(mut self, path: impl Into<String>) -> Self {
        self.playground_path = path.into();
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn register(
        self,
        builder: NivasaServerBuilder,
    ) -> Result<NivasaServerBuilder, RouteDispatchError> {
        let endpoint_path = self.endpoint_path;
        let playground_path = self.playground_path;
        let title = self.title;
        let executor = Arc::clone(&self.executor);
        let playground_html = graphql_playground_html(&title, &endpoint_path);
        let same_path_html = playground_html.clone();
        let separate_path_html = playground_html;

        let builder = builder.route(RouteMethod::Post, endpoint_path.clone(), move |request| {
            execute_graphql_request(request, Arc::clone(&executor))
        })?;

        if playground_path == endpoint_path {
            builder.route(RouteMethod::Get, endpoint_path, move |_| {
                NivasaResponse::html(same_path_html.clone())
            })
        } else {
            builder.route(RouteMethod::Get, playground_path, move |_| {
                NivasaResponse::html(separate_path_html.clone())
            })
        }
    }
}

fn execute_graphql_request(request: &NivasaRequest, executor: GraphQLExecutor) -> NivasaResponse {
    let request = match request.extract::<Json<GraphQLRequest>>() {
        Ok(Json(request)) => request,
        Err(error) => {
            let response = GraphQLResponse {
                data: None,
                errors: vec![GraphQLError::new(error.to_string())],
            };
            return NivasaResponse::new(StatusCode::BAD_REQUEST, Body::json(response.into_json()));
        }
    };

    let response = executor(request);
    NivasaResponse::json(response.into_json())
}

fn core_graphql_executor<Q, M, S>(module: GraphQLCoreModule<Q, M, S>) -> GraphQLExecutor
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    let module = Arc::new(module);

    Arc::new(move |request| {
        let request = to_core_graphql_request(request);
        let module = Arc::clone(&module);
        let response = block_on_graphql(async move { module.execute(request).await });
        from_core_graphql_response(response)
    })
}

fn to_core_graphql_request(request: GraphQLRequest) -> GraphQLCoreRequest {
    let mut payload = Map::new();
    payload.insert("query".to_string(), Value::String(request.query));

    if let Some(operation_name) = request.operation_name {
        payload.insert("operationName".to_string(), Value::String(operation_name));
    }

    if let Some(variables) = request.variables {
        payload.insert("variables".to_string(), variables);
    }

    if let Some(extensions) = request.extensions {
        payload.insert("extensions".to_string(), extensions);
    }

    serde_json::from_value(Value::Object(payload))
        .expect("GraphQL request envelope must convert into async-graphql request")
}

fn from_core_graphql_response(response: GraphQLCoreResponse) -> GraphQLResponse {
    let errors = response
        .errors
        .into_iter()
        .map(|error| GraphQLError {
            message: error.message,
            extensions: error.extensions.map(|extensions| {
                serde_json::to_value(extensions).expect("GraphQL error extensions must serialize")
            }),
        })
        .collect();

    GraphQLResponse {
        data: Some(
            serde_json::to_value(response.data).expect("GraphQL response data must serialize"),
        ),
        errors,
    }
}

fn block_on_graphql<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            tokio::runtime::RuntimeFlavor::CurrentThread => {
                std::thread::spawn(move || handle.block_on(future))
                    .join()
                    .expect("GraphQL runtime thread panicked")
            }
            _ => tokio::task::block_in_place(|| handle.block_on(future)),
        },
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("GraphQL runtime must build")
            .block_on(future),
    }
}

fn graphql_playground_html(title: &str, endpoint_path: &str) -> String {
    let html_title = escape_html(title);
    let endpoint =
        serde_json::to_string(endpoint_path).expect("playground endpoint must serialize");
    let endpoint_text = escape_html(endpoint_path);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html_title}</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f5f7fb;
      --panel: #ffffff;
      --border: #d6dbea;
      --ink: #0f172a;
      --muted: #475569;
      --accent: #1d4ed8;
    }}
    body {{
      margin: 0;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: linear-gradient(180deg, #eef2ff 0%, var(--bg) 100%);
      color: var(--ink);
    }}
    main {{
      max-width: 960px;
      margin: 0 auto;
      padding: 32px 20px 48px;
    }}
    .card {{
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 18px;
      box-shadow: 0 20px 45px rgba(15, 23, 42, 0.08);
      overflow: hidden;
    }}
    header {{
      padding: 24px 24px 16px;
      border-bottom: 1px solid var(--border);
    }}
    h1 {{
      margin: 0;
      font-size: 28px;
    }}
    p {{
      margin: 8px 0 0;
      color: var(--muted);
    }}
    .content {{
      display: grid;
      gap: 16px;
      padding: 24px;
    }}
    textarea {{
      width: 100%;
      min-height: 240px;
      border: 1px solid var(--border);
      border-radius: 14px;
      padding: 16px;
      font: inherit;
      resize: vertical;
      box-sizing: border-box;
    }}
    button {{
      justify-self: start;
      border: 0;
      border-radius: 999px;
      padding: 12px 18px;
      font: inherit;
      font-weight: 700;
      background: var(--accent);
      color: white;
      cursor: pointer;
    }}
    pre {{
      margin: 0;
      padding: 16px;
      border-radius: 14px;
      background: #0f172a;
      color: #e2e8f0;
      overflow: auto;
    }}
  </style>
</head>
<body>
  <main>
    <section class="card">
      <header>
        <h1>{html_title}</h1>
        <p>GraphQL playground for {endpoint_text}</p>
      </header>
      <div class="content">
        <textarea id="query">query Demo {{ __typename }}</textarea>
        <button id="run">Run query</button>
        <pre id="result">{{
  "hint": "Run a query to see the JSON response"
}}</pre>
      </div>
    </section>
  </main>
  <script>
    const endpoint = {endpoint};
    const query = document.getElementById("query");
    const result = document.getElementById("result");
    document.getElementById("run").addEventListener("click", async () => {{
      const response = await fetch(endpoint, {{
        method: "POST",
        headers: {{ "content-type": "application/json" }},
        body: JSON.stringify({{ query: query.value }})
      }});
      result.textContent = await response.text();
    }});
  </script>
</body>
</html>"#,
    )
}

fn escape_html(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '"' => "&quot;".chars().collect::<Vec<_>>(),
            '\'' => "&#39;".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}
