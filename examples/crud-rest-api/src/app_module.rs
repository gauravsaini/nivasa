use nivasa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserDto {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserDto {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: u64,
    pub name: String,
}

#[injectable]
pub struct UsersService;

impl UsersService {
    pub fn list(&self) -> Vec<UserDto> {
        vec![UserDto {
            id: 1,
            name: "Ada".to_string(),
        }]
    }

    pub fn get(&self, id: u64) -> Option<UserDto> {
        Some(UserDto {
            id,
            name: "Ada".to_string(),
        })
    }

    pub fn create(&self, dto: CreateUserDto) -> UserDto {
        UserDto {
            id: 2,
            name: dto.name,
        }
    }

    pub fn update(&self, id: u64, dto: UpdateUserDto) -> Option<UserDto> {
        Some(UserDto {
            id,
            name: dto.name.unwrap_or_else(|| "Ada".to_string()),
        })
    }

    pub fn delete(&self, id: u64) -> bool {
        id > 0
    }
}

#[controller("/users")]
pub struct UsersController;

#[impl_controller]
impl UsersController {
    #[get("/")]
    pub fn list(&self) -> serde_json::Value {
        let service = UsersService;
        serde_json::json!({"items": service.list()})
    }

    #[post("/")]
    pub fn create(&self, #[body] dto: CreateUserDto) -> serde_json::Value {
        let service = UsersService;
        serde_json::json!(service.create(dto))
    }

    #[get("/:id")]
    pub fn get(&self, #[param("id")] id: u64) -> serde_json::Value {
        let service = UsersService;
        serde_json::json!(service.get(id))
    }

    #[put("/:id")]
    pub fn update(&self, #[param("id")] id: u64, #[body] dto: UpdateUserDto) -> serde_json::Value {
        let service = UsersService;
        serde_json::json!(service.update(id, dto))
    }

    #[delete("/:id")]
    pub fn delete(&self, #[param("id")] id: u64) -> serde_json::Value {
        let service = UsersService;
        serde_json::json!({ "deleted": service.delete(id) })
    }
}

#[module({
    controllers: [UsersController],
    providers: [UsersService],
})]
pub struct AppModule;
