#![allow(unused_imports)]
#![allow(dead_code)]

use std::any::TypeId;
use std::time::Duration;

use nivasa_macros::{
    all, body, catch, catch_all, connected_socket, controller, cron, custom_param, delete, file,
    files, get, guard, head, header, headers, http_code, impl_controller, injectable, interceptor,
    interval, ip, message_body, middleware, module, mutation, on_event, options, param, patch,
    pipe, post, put, query, req, res, resolver, roles, scxml_handler, session, set_metadata,
    skip_throttle, subscribe_message, subscription, throttle, timeout, use_filters,
    websocket_gateway, ConfigSchema as DeriveConfigSchema, Dto, PartialDto,
};
use nivasa_scheduling::SchedulePattern;

struct CustomExtractor;
struct TrimPipe;
struct ImportedModule;
struct RoomGuard;
struct AuditInterceptor;

#[catch(std::io::Error)]
struct IoFilter;

#[catch_all]
struct DefaultFilter;

#[middleware]
struct LoggingMiddleware;

#[injectable]
struct Service;

#[controller("/surface")]
#[guard(RoomGuard)]
#[roles("admin")]
#[interceptor(AuditInterceptor)]
#[use_filters(IoFilter, DefaultFilter)]
#[set_metadata(key = "scope", value = "surface")]
#[throttle(limit = 20, ttl = 60)]
struct SurfaceController;

#[impl_controller]
impl SurfaceController {
    #[get("/list")]
    fn list(&self) {}

    #[post("/create")]
    #[http_code(201)]
    #[header("x-surface", "create")]
    fn create(
        &self,
        #[body] body: String,
        #[param("id")] id: String,
        #[headers] headers: String,
        #[req] request: String,
        #[res] response: String,
        #[custom_param(CustomExtractor)] extractor: String,
        #[ip] ip: String,
        #[session] session: String,
        #[file] file: String,
        #[files] files: String,
    ) {
        let _ = (
            body, id, headers, request, response, extractor, ip, session, file, files,
        );
    }

    #[put("/update")]
    fn update(&self) {}

    #[delete("/archive")]
    fn archive(&self) {}

    #[patch("/rename")]
    fn rename(&self) {}

    #[head("/health")]
    fn health(&self) {}

    #[options("/capabilities")]
    fn capabilities(&self) {}

    #[all("/anything")]
    #[skip_throttle]
    fn anything(&self) {}

    #[post("/publish")]
    fn publish(
        &self,
        #[pipe(TrimPipe)]
        #[message_body("payload")]
        payload: String,
        #[message_body] raw: String,
        #[connected_socket] socket: String,
    ) {
        let _ = (payload, raw, socket);
    }
}

#[module({
    imports: [ImportedModule],
    controllers: [SurfaceController],
    providers: [Service],
    exports: [Service],
    middlewares: [LoggingMiddleware],
})]
struct SurfaceModule;

#[websocket_gateway({ path: "/ws", namespace: "/surface" })]
struct SurfaceGateway;

impl SurfaceGateway {
    #[on_event("user.created")]
    fn on_user_created(&self) {}

    #[subscribe_message("chat.join")]
    fn on_chat_join(&self) {}

    #[query("allUsers")]
    #[resolver("users")]
    fn users(&self) {}

    #[mutation("createUser")]
    fn create_user(&self) {}

    #[subscription("userCreated")]
    fn user_created(&self) {}

    #[cron("0 */5 * * * *")]
    fn cron_job(&self) {}

    #[interval(5000)]
    fn tick(&self) {}

    #[timeout(1000)]
    fn once(&self) {}
}

#[scxml_handler(statechart = "request", state = "guard_chain")]
fn guard_chain() {}

pub trait ConfigSchema {
    fn required_keys() -> &'static [&'static str] {
        &[]
    }

    fn defaults() -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

#[derive(Dto)]
struct CreateUserDto {
    #[is_string]
    #[is_not_empty]
    name: String,
}

#[derive(PartialDto)]
struct PatchUserDto {
    #[is_optional]
    #[is_string]
    name: Option<String>,
}

#[derive(DeriveConfigSchema)]
struct AppConfig {
    host: String,
    #[schema(default = "3000")]
    port: String,
}

#[test]
fn umbrella_controller_surface_exposes_metadata() {
    assert_eq!(
        SurfaceController::__nivasa_controller_guards(),
        vec!["RoomGuard"]
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_roles(),
        vec!["admin"]
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_interceptors(),
        vec!["AuditInterceptor"]
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_filters(),
        vec!["IoFilter", "DefaultFilter"]
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_set_metadata(),
        vec![("scope", "surface")]
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_routes(),
        vec![
            ("GET", "/surface/list".to_string(), "list"),
            ("POST", "/surface/create".to_string(), "create"),
            ("PUT", "/surface/update".to_string(), "update"),
            ("DELETE", "/surface/archive".to_string(), "archive"),
            ("PATCH", "/surface/rename".to_string(), "rename"),
            ("HEAD", "/surface/health".to_string(), "health"),
            (
                "OPTIONS",
                "/surface/capabilities".to_string(),
                "capabilities"
            ),
            ("ALL", "/surface/anything".to_string(), "anything"),
            ("POST", "/surface/publish".to_string(), "publish"),
        ],
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_response_metadata(),
        vec![
            ("list", None, vec![]),
            ("create", Some(201), vec![("x-surface", "create")]),
            ("update", None, vec![]),
            ("archive", None, vec![]),
            ("rename", None, vec![]),
            ("health", None, vec![]),
            ("capabilities", None, vec![]),
            ("anything", None, vec![]),
            ("publish", None, vec![]),
        ],
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_parameter_metadata(),
        vec![
            ("list", vec![]),
            (
                "create",
                vec![
                    ("body", None),
                    ("param", Some("id")),
                    ("headers", None),
                    ("req", None),
                    ("res", None),
                    ("custom_param", Some("CustomExtractor")),
                    ("ip", None),
                    ("session", None),
                    ("file", None),
                    ("files", None),
                ],
            ),
            ("update", vec![]),
            ("archive", vec![]),
            ("rename", vec![]),
            ("health", vec![]),
            ("capabilities", vec![]),
            ("anything", vec![]),
            (
                "publish",
                vec![
                    ("message_body", Some("payload")),
                    ("message_body", None),
                    ("connected_socket", None),
                ],
            ),
        ],
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_parameter_pipe_metadata(),
        vec![("publish", vec![vec!["TrimPipe"], vec![], vec![]],)],
    );
    assert_eq!(
        SurfaceController::__nivasa_controller_throttle_default(),
        Some((20, 60))
    );
    assert!(!SurfaceController::__nivasa_controller_skip_throttle());
    assert_eq!(
        SurfaceController::__nivasa_controller_throttle_metadata(),
        vec![
            ("list", Some((20, 60)), false),
            ("create", Some((20, 60)), false),
            ("update", Some((20, 60)), false),
            ("archive", Some((20, 60)), false),
            ("rename", Some((20, 60)), false),
            ("health", Some((20, 60)), false),
            ("capabilities", Some((20, 60)), false),
            ("anything", None, true),
            ("publish", Some((20, 60)), false),
        ],
    );
}

#[test]
fn umbrella_module_and_misc_surface_exposes_metadata() {
    assert_eq!(
        SurfaceModule::__nivasa_module_imports(),
        vec![TypeId::of::<ImportedModule>()]
    );
    assert_eq!(
        SurfaceModule::__nivasa_module_controllers(),
        vec![TypeId::of::<SurfaceController>()]
    );
    assert_eq!(
        SurfaceModule::__nivasa_module_providers(),
        vec![TypeId::of::<Service>()]
    );
    assert_eq!(
        SurfaceModule::__nivasa_module_exports(),
        vec![TypeId::of::<Service>()]
    );
    assert_eq!(
        SurfaceModule::__nivasa_module_middlewares(),
        vec![TypeId::of::<LoggingMiddleware>()]
    );

    assert_eq!(IoFilter::__nivasa_filter_exception(), "std::io::Error");
    assert!(DefaultFilter::__nivasa_filter_catch_all());
    assert_eq!(
        LoggingMiddleware::__nivasa_middleware_name(),
        "LoggingMiddleware"
    );
    assert_eq!(
        SurfaceGateway::__nivasa_websocket_gateway_metadata(),
        ("/ws", Some("/surface"))
    );
    assert_eq!(
        SurfaceGateway::__nivasa_on_event_metadata_for_on_user_created(),
        ("on_user_created", "user.created"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_subscribe_message_metadata_for_on_chat_join(),
        ("on_chat_join", "chat.join"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_graphql_query_metadata_for_users(),
        ("users", "allUsers"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_graphql_resolver_metadata_for_users(),
        ("users", "users"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_graphql_mutation_metadata_for_create_user(),
        ("create_user", "createUser"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_graphql_subscription_metadata_for_user_created(),
        ("user_created", "userCreated"),
    );
    assert_eq!(
        SurfaceGateway::__nivasa_cron_metadata_for_cron_job(),
        SchedulePattern::Cron {
            expression: "0 */5 * * * *".to_string(),
        }
    );
    assert_eq!(
        SurfaceGateway::__nivasa_interval_metadata_for_tick(),
        SchedulePattern::Interval {
            every: Duration::from_millis(5000),
        }
    );
    assert_eq!(
        SurfaceGateway::__nivasa_timeout_metadata_for_once(),
        SchedulePattern::Timeout {
            delay: Duration::from_millis(1000),
        }
    );

    guard_chain();
}

#[test]
fn umbrella_validation_and_config_derives_expand() {
    let create = CreateUserDto {
        name: String::from("alice"),
    };
    let patch = PatchUserDto {
        name: Some(String::from("bob")),
    };

    assert_eq!(create.name, "alice");
    assert_eq!(patch.name.as_deref(), Some("bob"));
    assert_eq!(AppConfig::required_keys(), &["host"]);
    assert_eq!(AppConfig::defaults(), &[("port", "3000")]);
}
