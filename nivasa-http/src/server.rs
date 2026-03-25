use crate::{Body, NivasaRequest, NivasaResponse, RequestPipeline};
use bytes::Bytes;
use http::{header::ALLOW, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use nivasa_routing::{
    parse_api_version_accept, parse_api_version_header, RouteDispatchError, RouteDispatchOutcome,
    RouteDispatchRegistry, RouteMethod,
};
use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::{net::TcpListener, sync::oneshot, task::JoinSet};

type RouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;

/// Minimal HTTP transport shell for Nivasa.
pub struct NivasaServer {
    routes: RouteDispatchRegistry<RouteHandler>,
    shutdown: Option<oneshot::Receiver<()>>,
}

/// Builder for [`NivasaServer`].
pub struct NivasaServerBuilder {
    routes: RouteDispatchRegistry<RouteHandler>,
    shutdown: Option<oneshot::Receiver<()>>,
}

impl NivasaServer {
    /// Create a new server builder.
    pub fn builder() -> NivasaServerBuilder {
        NivasaServerBuilder::new()
    }

    /// Start listening for HTTP requests.
    pub async fn listen(mut self, host: impl AsRef<str>, port: u16) -> io::Result<()> {
        let addr = socket_addr(host.as_ref(), port)?;
        let listener = TcpListener::bind(addr).await?;
        let mut shutdown = shutdown_future(self.shutdown.take());
        let routes = self.routes;
        let mut connections = JoinSet::new();

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    break;
                }
                accept = listener.accept() => {
                    let (stream, _) = accept?;
                    let routes = routes.clone();

                    connections.spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |request| {
                            let routes = routes.clone();
                            async move { handle_request(request, routes).await }
                        });

                        let builder = AutoBuilder::new(TokioExecutor::new());
                        let _ = builder.serve_connection(io, service).await;
                    });
                }
            }
        }

        while connections.join_next().await.is_some() {}
        Ok(())
    }
}

impl NivasaServerBuilder {
    fn new() -> Self {
        Self {
            routes: RouteDispatchRegistry::new(),
            shutdown: None,
        }
    }

    /// Register a request handler for a route.
    pub fn route(
        mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        let handler: RouteHandler = Arc::new(handler);
        self.routes.register_pattern(method, path, handler)?;
        Ok(self)
    }

    /// Register a route that is selected by `X-API-Version`.
    pub fn route_header_versioned(
        mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        let handler: RouteHandler = Arc::new(handler);
        self.routes
            .register_header_versioned_route(method, version, path, handler)?;
        Ok(self)
    }

    /// Register a route that is selected by an `Accept` media type version.
    pub fn route_media_type_versioned(
        mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        let handler: RouteHandler = Arc::new(handler);
        self.routes
            .register_media_type_versioned_route(method, version, path, handler)?;
        Ok(self)
    }

    /// Provide a custom shutdown signal for tests or embeddings.
    pub fn shutdown_signal(mut self, shutdown: oneshot::Receiver<()>) -> Self {
        self.shutdown = Some(shutdown);
        self
    }

    /// Finalize the server.
    pub fn build(self) -> NivasaServer {
        NivasaServer {
            routes: self.routes,
            shutdown: self.shutdown,
        }
    }
}

async fn handle_request(
    request: hyper::Request<Incoming>,
    routes: RouteDispatchRegistry<RouteHandler>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let (parts, body) = request.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return Ok(build_response(
                StatusCode::BAD_REQUEST,
                NivasaResponse::new(StatusCode::BAD_REQUEST, Body::text("invalid request body")),
            ));
        }
    };

    let body = if body.is_empty() {
        Body::empty()
    } else {
        Body::bytes(body.to_vec())
    };

    let request = NivasaRequest::from_http(Request::from_parts(parts, body));
    let mut pipeline = RequestPipeline::new(request);

    if pipeline.parse_request().is_err() {
        return Ok(build_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline parse transition failed"),
            ),
        ));
    }

    if pipeline.complete_middleware().is_err() {
        return Ok(build_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline middleware transition failed"),
            ),
        ));
    }

    let versioned_routes = versioned_routes_for_request(pipeline.request(), &routes);

    let response = match pipeline.match_route(&versioned_routes) {
        Ok(RouteDispatchOutcome::Matched(entry)) => (entry.value)(pipeline.request()),
        Ok(RouteDispatchOutcome::NotFound) => {
            NivasaResponse::new(StatusCode::NOT_FOUND, Body::text("not found"))
        }
        Ok(RouteDispatchOutcome::MethodNotAllowed { allowed_methods, .. }) => {
            NivasaResponse::new(
                StatusCode::METHOD_NOT_ALLOWED,
                Body::text("method not allowed"),
            )
            .with_header(ALLOW.as_str(), allowed_methods.join(", "))
        }
        Err(_) => NivasaResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Body::text("request pipeline route transition failed"),
        ),
    };

    Ok(build_response(response.status(), response))
}

fn versioned_routes_for_request(
    request: &NivasaRequest,
    routes: &RouteDispatchRegistry<RouteHandler>,
) -> RouteDispatchRegistry<RouteHandler> {
    let version = request
        .headers()
        .get("X-API-Version")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_api_version_header)
        .or_else(|| {
            request
                .headers()
                .get(http::header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_api_version_accept)
        });

    let mut selected = RouteDispatchRegistry::new();

    let mut saw_versioned_match = false;
    if let Some(version) = version.as_deref() {
        for entry in routes.iter() {
            if entry.version.as_deref() == Some(version) && entry.pattern.matches(request.path()) {
                saw_versioned_match = true;
            }
        }
    }

    for entry in routes.iter() {
        let matches_path = entry.pattern.matches(request.path());
        if !matches_path {
            continue;
        }

        let should_include = match version.as_deref() {
            Some(version) if saw_versioned_match => entry.version.as_deref() == Some(version),
            Some(_) => entry.version.is_none(),
            None => entry.version.is_none(),
        };

        if should_include {
            let _ = selected.register(
                entry.method.clone(),
                entry.pattern.clone(),
                entry.value.clone(),
            );
        }
    }

    selected
}

fn build_response(status: StatusCode, response: NivasaResponse) -> Response<Full<Bytes>> {
    let mut hyper_response = Response::new(Full::new(Bytes::from(response.body().as_bytes())));
    *hyper_response.status_mut() = status;
    *hyper_response.headers_mut() = response.headers().clone();
    hyper_response
}

fn socket_addr(host: &str, port: u16) -> io::Result<SocketAddr> {
    format!("{}:{}", host.trim(), port).parse().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid listen address: {err}"),
        )
    })
}

fn shutdown_future(
    shutdown: Option<oneshot::Receiver<()>>,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    match shutdown {
        Some(shutdown) => Box::pin(async move {
            let _ = shutdown.await;
        }),
        None => Box::pin(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};

                let mut sigterm = signal(SignalKind::terminate())
                    .expect("SIGTERM signal handler must be available");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
            }

            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        }),
    }
}
