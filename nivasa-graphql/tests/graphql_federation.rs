use async_graphql::{value, EmptyMutation, EmptySubscription, Object};
use nivasa_graphql::GraphQLModule;

struct QueryRoot;

#[derive(Clone)]
struct Product {
    upc: String,
    name: String,
}

#[Object]
impl Product {
    async fn upc(&self) -> &str {
        &self.upc
    }

    async fn name(&self) -> &str {
        &self.name
    }
}

#[Object]
impl QueryRoot {
    async fn ping(&self) -> &str {
        "pong"
    }

    #[graphql(entity)]
    async fn product_by_upc(&self, upc: String) -> Product {
        Product {
            upc,
            name: "GraphQL Federation".to_string(),
        }
    }
}

#[tokio::test]
async fn federated_module_exposes_service_sdl_and_entities() {
    let module = GraphQLModule::federated(QueryRoot, EmptyMutation, EmptySubscription);

    let service_response = module.execute("{ _service { sdl } }").await;
    let service_json = service_response.data.into_json().unwrap();
    let sdl = service_json["_service"]["sdl"].as_str().unwrap();

    assert!(sdl.contains("type Product @key(fields: \"upc\")"));
    assert!(sdl.contains("type Query"));

    let entity_response = module
        .execute(
            r#"{
                _entities(representations: [{__typename: "Product", upc: "B00005N5PF"}]) {
                    __typename
                    ... on Product {
                        upc
                        name
                    }
                }
            }"#,
        )
        .await;

    assert_eq!(
        entity_response.data,
        value!({
            "_entities": [
                {
                    "__typename": "Product",
                    "upc": "B00005N5PF",
                    "name": "GraphQL Federation"
                }
            ]
        })
    );
    assert!(entity_response.errors.is_empty());
}
