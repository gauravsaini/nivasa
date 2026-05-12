//! # nivasa-graphql
//!
//! GraphQL runtime support for Nivasa.
//!
//! This crate wraps [`async_graphql`] with a small Nivasa module that can own
//! a schema, expose it through the DI container, and execute GraphQL requests
//! directly.
//!
//! # Example
//!
//! ```rust
//! use async_graphql::{EmptyMutation, EmptySubscription, Object};
//! use nivasa_graphql::GraphQLModule;
//!
//! struct QueryRoot;
//!
//! #[Object]
//! impl QueryRoot {
//!     async fn hello(&self) -> &str {
//!         "world"
//!     }
//! }
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let module = GraphQLModule::new(QueryRoot, EmptyMutation, EmptySubscription);
//! let response = module.execute("{ hello }").await;
//!
//! assert_eq!(response.data, async_graphql::value!({ "hello": "world" }));
//! # });
//! ```

use async_trait::async_trait;
use nivasa_core::di::{DependencyContainer, DiError};
use nivasa_core::module::{Module, ModuleMetadata};
use std::any::TypeId;

pub use async_graphql::{
    EmptyMutation, EmptySubscription, Request as GraphQLRequest, Response as GraphQLResponse,
    Schema as GraphQLSchema,
};
use async_graphql::{ObjectType, SubscriptionType};

/// GraphQL module wrapper backed by an async-graphql schema.
pub struct GraphQLModule<Q, M = EmptyMutation, S = EmptySubscription>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    schema: GraphQLSchema<Q, M, S>,
}

impl<Q, M, S> GraphQLModule<Q, M, S>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    /// Build a GraphQL module from concrete root objects.
    pub fn new(query: Q, mutation: M, subscription: S) -> Self {
        Self {
            schema: GraphQLSchema::build(query, mutation, subscription).finish(),
        }
    }

    /// Build a GraphQL module with Apollo Federation enabled.
    #[must_use]
    pub fn federated(query: Q, mutation: M, subscription: S) -> Self {
        Self {
            schema: GraphQLSchema::build(query, mutation, subscription)
                .enable_federation()
                .finish(),
        }
    }

    /// Wrap an existing async-graphql schema.
    #[must_use]
    pub fn from_schema(schema: GraphQLSchema<Q, M, S>) -> Self {
        Self { schema }
    }

    /// Borrow the underlying schema.
    pub fn schema(&self) -> &GraphQLSchema<Q, M, S> {
        &self.schema
    }

    /// Execute one GraphQL request against the schema.
    pub async fn execute(&self, request: impl Into<GraphQLRequest>) -> GraphQLResponse {
        self.schema.execute(request.into()).await
    }
}

#[async_trait]
impl<Q, M, S> Module for GraphQLModule<Q, M, S>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    fn metadata(&self) -> ModuleMetadata {
        let schema_type = TypeId::of::<GraphQLSchema<Q, M, S>>();

        ModuleMetadata::new()
            .with_providers(vec![schema_type])
            .with_exports(vec![schema_type])
            .with_global(true)
    }

    async fn configure(&self, container: &DependencyContainer) -> Result<(), DiError> {
        container.register_value(self.schema.clone()).await;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{EmptyMutation, EmptySubscription, Object};

    struct Query;

    #[Object]
    impl Query {
        async fn ping(&self) -> &str {
            "pong"
        }
    }

    // ── GraphQLModule::new ────────────────────────────────────────────────────

    #[tokio::test]
    async fn graphql_module_new_executes_query() {
        let module = GraphQLModule::new(Query, EmptyMutation, EmptySubscription);
        let response = module.execute("{ ping }").await;
        assert!(response.errors.is_empty(), "errors: {:?}", response.errors);
        assert_eq!(response.data, async_graphql::value!({ "ping": "pong" }));
    }

    // ── GraphQLModule::from_schema ────────────────────────────────────────────

    #[tokio::test]
    async fn graphql_module_from_schema_borrows_schema() {
        let schema = GraphQLSchema::build(Query, EmptyMutation, EmptySubscription).finish();
        let module = GraphQLModule::from_schema(schema);
        let response = module.execute("{ ping }").await;
        assert!(response.errors.is_empty());
    }

    #[tokio::test]
    async fn graphql_module_schema_accessor_returns_schema() {
        let module = GraphQLModule::new(Query, EmptyMutation, EmptySubscription);
        // Should return a schema reference; executing a query on it directly confirms it works
        let response = module.schema().execute("{ ping }").await;
        assert!(response.errors.is_empty());
    }

    // ── GraphQLModule::federated ───────────────────────────────────────────────

    #[tokio::test]
    async fn graphql_module_federated_executes_query() {
        let module = GraphQLModule::federated(Query, EmptyMutation, EmptySubscription);
        let response = module.execute("{ ping }").await;
        assert!(response.errors.is_empty(), "errors: {:?}", response.errors);
    }

    // ── Module::configure ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn graphql_module_configure_registers_schema_in_container() {
        use nivasa_core::di::DependencyContainer;

        let module = GraphQLModule::new(Query, EmptyMutation, EmptySubscription);
        let container = DependencyContainer::new();
        module.configure(&container).await.unwrap();

        let resolved = container
            .resolve::<GraphQLSchema<Query, EmptyMutation, EmptySubscription>>()
            .await;
        assert!(resolved.is_ok(), "schema must be resolvable from container");
    }

    // ── Module::metadata ──────────────────────────────────────────────────────

    #[test]
    fn graphql_module_metadata_has_global_flag() {
        let module = GraphQLModule::new(Query, EmptyMutation, EmptySubscription);
        let meta = module.metadata();
        assert!(meta.is_global, "GraphQL module must export globally");
    }
}
