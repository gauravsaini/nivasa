use nivasa_macros::{interceptor, mutation, query, resolver, subscription, websocket_gateway};
use trybuild::TestCases;

#[allow(dead_code)]
struct QueryGuard;
#[allow(dead_code)]
struct AuditInterceptor;
#[allow(dead_code)]
struct MetricsInterceptor;

#[websocket_gateway("/graphql")]
struct GraphqlGateway;

impl GraphqlGateway {
    #[nivasa_macros::guard(QueryGuard)]
    #[interceptor(AuditInterceptor, MetricsInterceptor)]
    #[query("allUsers")]
    #[resolver("users")]
    fn users(&self) -> Vec<String> {
        vec!["alice".to_string(), "bob".to_string()]
    }

    #[mutation("createUser")]
    fn create_user(&self, name: String) -> String {
        format!("created:{name}")
    }

    #[subscription("userCreated")]
    fn user_created(&self, id: String) -> String {
        format!("subscribed:{id}")
    }
}

struct DirectAttrGraphqlGateway;

impl DirectAttrGraphqlGateway {
    #[resolver("directUsers")]
    #[nivasa_macros::guard(QueryGuard)]
    #[interceptor(AuditInterceptor)]
    fn users(&self) {}
}

#[test]
fn graphql_macros_emit_handler_metadata() {
    assert_eq!(
        GraphqlGateway::__nivasa_graphql_query_metadata_for_users(),
        ("users", "allUsers")
    );
    assert_eq!(
        GraphqlGateway::__nivasa_graphql_resolver_metadata_for_users(),
        ("users", "users")
    );
    assert_eq!(
        GraphqlGateway::__nivasa_graphql_resolver_guard_metadata_for_users(),
        vec!["QueryGuard"],
    );
    assert_eq!(
        GraphqlGateway::__nivasa_graphql_resolver_interceptor_metadata_for_users(),
        vec!["AuditInterceptor", "MetricsInterceptor"],
    );

    assert_eq!(
        GraphqlGateway::__nivasa_graphql_mutation_metadata_for_create_user(),
        ("create_user", "createUser")
    );
    assert_eq!(
        GraphqlGateway::__nivasa_graphql_subscription_metadata_for_user_created(),
        ("user_created", "userCreated")
    );

    let gateway = GraphqlGateway;
    assert_eq!(
        gateway.users(),
        vec!["alice".to_string(), "bob".to_string()]
    );
    assert_eq!(gateway.create_user("delta".to_string()), "created:delta");
    assert_eq!(gateway.user_created("42".to_string()), "subscribed:42");

    assert_eq!(
        DirectAttrGraphqlGateway::__nivasa_graphql_resolver_guard_metadata_for_users(),
        vec!["QueryGuard"],
    );
    assert_eq!(
        DirectAttrGraphqlGateway::__nivasa_graphql_resolver_interceptor_metadata_for_users(),
        vec!["AuditInterceptor"],
    );
    DirectAttrGraphqlGateway.users();
}

#[allow(dead_code)]
struct DocMarkerGuard;
#[allow(dead_code)]
struct DocMarkerInterceptorA;
#[allow(dead_code)]
struct DocMarkerInterceptorB;

struct DocMarkerGateway;

impl DocMarkerGateway {
    /// nivasa-guard: DocMarkerGuard
    /// nivasa-interceptor: DocMarkerInterceptorA, DocMarkerInterceptorB
    #[resolver("docUsers")]
    fn users(&self) -> Vec<String> {
        vec!["carol".to_string()]
    }
}

#[test]
fn graphql_macros_parse_doc_markers() {
    assert_eq!(
        DocMarkerGateway::__nivasa_graphql_resolver_guard_metadata_for_users(),
        vec!["DocMarkerGuard"],
    );
    assert_eq!(
        DocMarkerGateway::__nivasa_graphql_resolver_interceptor_metadata_for_users(),
        vec!["DocMarkerInterceptorA", "DocMarkerInterceptorB"],
    );

    let gateway = DocMarkerGateway;
    assert_eq!(gateway.users(), vec!["carol".to_string()]);
}

#[test]
fn graphql_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/graphql_invalid_target.rs");
    t.compile_fail("tests/trybuild/graphql_invalid_args.rs");
    t.compile_fail("tests/trybuild/graphql_empty_name.rs");
    t.compile_fail("tests/trybuild/graphql_guard_requires_arg.rs");
    t.compile_fail("tests/trybuild/graphql_interceptor_requires_arg.rs");
    t.compile_fail("tests/trybuild/graphql_invalid_guard_marker.rs");
    t.compile_fail("tests/trybuild/graphql_invalid_interceptor_marker.rs");
}
