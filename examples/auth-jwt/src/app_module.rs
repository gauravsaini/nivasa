use nivasa::prelude::*;
use std::sync::Arc;

pub const SESSION_TOKEN: &str = "Bearer header.payload.signature";

#[allow(dead_code)]
struct AuthFlowGuard;

impl Guard for AuthFlowGuard {
    fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            let request = context
                .request::<NivasaRequest>()
                .expect("guard context must carry the request");

            if request.path() == "/auth/login" {
                return Ok(true);
            }

            Ok(request
                .header("authorization")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value == SESSION_TOKEN))
        })
    }
}

pub fn login_response() -> NivasaResponse {
    NivasaResponse::text(format!(
        "accessToken={SESSION_TOKEN}; tokenType=Bearer; example=login -> JWT -> protected route"
    ))
}

pub fn profile_response(request: &NivasaRequest) -> NivasaResponse {
    let authorized = request
        .header("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == SESSION_TOKEN);

    if authorized {
        NivasaResponse::text("profile=protected profile; subject=ada; guard=auth-jwt")
    } else {
        NivasaResponse::new(
            HttpStatus::Forbidden.to_http_status_code(),
            Body::text("error=missing or invalid bearer token"),
        )
    }
}

pub fn resolve_route_handler(route: &str) -> Option<nivasa::application::AppRouteHandler> {
    match route {
        "login" => Some(Arc::new(|_| login_response())),
        "profile" => Some(Arc::new(profile_response)),
        _ => None,
    }
}

#[allow(dead_code)]
#[controller("/auth")]
pub struct LoginController;

#[allow(dead_code)]
#[impl_controller]
impl LoginController {
    #[get("/login")]
    pub fn login(&self) -> &'static str {
        SESSION_TOKEN
    }
}

#[allow(dead_code)]
#[controller("/auth")]
#[guard(AuthFlowGuard)]
pub struct ProfileController;

#[allow(dead_code)]
#[impl_controller]
impl ProfileController {
    #[get("/profile")]
    pub fn profile(&self) -> &'static str {
        "protected profile"
    }
}

#[module({
    controllers: [LoginController, ProfileController],
})]
pub struct AppModule;
