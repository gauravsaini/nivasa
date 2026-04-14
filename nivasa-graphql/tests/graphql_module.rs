use async_graphql::{value, EmptyMutation, EmptySubscription, Object};
use nivasa_core::{DependencyContainer, Module};
use nivasa_graphql::{GraphQLModule, GraphQLSchema};
use std::any::TypeId;

struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> &str {
        "world"
    }

    async fn answer(&self) -> i32 {
        42
    }
}

#[tokio::test]
async fn graphql_module_executes_queries() {
    let module = GraphQLModule::new(QueryRoot, EmptyMutation, EmptySubscription);

    let response = module.execute("{ hello answer }").await;

    assert_eq!(
        response.data,
        value!({
            "hello": "world",
            "answer": 42
        })
    );
    assert!(response.errors.is_empty());
}

#[tokio::test]
async fn graphql_module_registers_schema_in_di_container() {
    let module = GraphQLModule::new(QueryRoot, EmptyMutation, EmptySubscription);
    let container = DependencyContainer::new();

    module.configure(&container).await.unwrap();

    let schema = container
        .resolve::<GraphQLSchema<QueryRoot, EmptyMutation, EmptySubscription>>()
        .await
        .unwrap();
    let response = schema.execute("{ answer }").await;

    assert_eq!(response.data, value!({ "answer": 42 }));
}

#[test]
fn graphql_module_metadata_exposes_schema_type() {
    let module = GraphQLModule::new(QueryRoot, EmptyMutation, EmptySubscription);
    let schema_type = TypeId::of::<GraphQLSchema<QueryRoot, EmptyMutation, EmptySubscription>>();

    let metadata = module.metadata();

    assert!(metadata.is_global);
    assert!(metadata.providers.contains(&schema_type));
    assert!(metadata.exports.contains(&schema_type));
}
