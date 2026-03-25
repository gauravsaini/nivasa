# Nivasa - Rust-Based NestJS Alternative - Implementation Plan

**Nivasa** (निवास) - Sanskrit for "home, dwelling, abode, nest"

A modular, decorator-based Rust web framework that provides 100% NestJS pattern compliance.

## Goal
Build a Rust web framework that is 100% compliant with NestJS patterns, including modules, dependency injection, decorators (via procedural macros), guards, interceptors, pipes, filters, and controllers.

## Why This is Needed
- NestJS provides excellent productivity with its decorator-based, modular architecture
- Rust offers performance and safety but lacks a framework with NestJS-like ergonomics
- Existing Rust frameworks (Actix, Axum, Poem, Rocket, Pavex) have good features but don't match NestJS patterns
- No existing Rust framework has full DI + module system + decorator-based routing combined

## About the Name "Nivasa"
**Nivasa** (निवास) comes from Sanskrit, meaning "home, dwelling, abode, nest."

This name is perfect because:
- It captures the essence of "Nest" from NestJS
- Represents a welcoming home for your application code
- Short (6 letters), easy to pronounce and remember
- Carries the ancient wisdom of Sanskrit while being modern and accessible
- Reflects the framework's role as a comfortable, structured home for your web applications

## Research Summary

### Existing Rust Frameworks Analyzed

1. **Actix Web**
   - Pros: Extremely fast, powerful, mature
   - Cons: No built-in DI, not module-oriented, different paradigm
   - Gap: Need to add DI container and module system

2. **Axum**
   - Pros: Ergonomic, uses tower ecosystem, extractors
   - Cons: No built-in DI, no decorator-based routing
   - Gap: Need full DI + module system + procedural macros for routing

3. **Poem**
   - Pros: Has some DI concepts, modular
   - Cons: Limited DI, not NestJS-like pattern matching
   - Gap: Need to enhance DI and add decorator system

4. **Rocket**
   - Pros: Type-safe, good routing
   - Cons: No DI container, different approach
   - Gap: Need DI + module system

5. **Pavex**
   - Pros: Compiler-based, Blueprint concept similar to modules
   - Cons: In closed beta, different paradigm from NestJS
   - Gap: Need to align more closely with NestJS patterns

### Dependency Injection Crates Analyzed

1. **springtime-di** (26,584 downloads)
   - Pros: Spring-inspired, automatic component discovery, component-based
   - Cons: Not web framework specific, requires integration
   - Potential: Could be used as DI foundation

2. **blackbox_di** (2,978 downloads)
   - Pros: Compile-time DI
   - Cons: Less mature, fewer features
   - Potential: Could be enhanced

3. **fundle** (834 downloads)
   - Pros: Compile-time safe, Microsoft-backed
   - Cons: Less community adoption
   - Potential: Good for safety-critical applications

### Key Findings
- No existing Rust framework provides NestJS-like decorator-based architecture
- DI containers exist but need web framework integration
- Procedural macros can replicate decorator patterns
- Module system needs to be built from scratch

## Proposed Architecture

### Core Components

#### 1. Module System (NestJS: @Module)
```rust
#[module({
    imports: [AuthModule, DatabaseModule],
    controllers: [UserController],
    providers: [UserService, AuthService],
    exports: [UserService]
})
pub struct UserModule;
```
- Modules organize application structure
- Support imports, exports, controllers, providers
- Nested modules with dependency resolution
- Module lifecycle hooks (onInit, onDestroy)

#### 2. Dependency Injection Container
```rust
#[injectable]
pub struct UserService {
    #[inject]
    repository: Arc<UserRepository>,
}

#[controller]
pub struct UserController {
    #[inject]
    service: Arc<UserService>,
}
```
- Compile-time DI with zero runtime overhead
- Support for singleton, scoped, transient lifetimes
- Circular dependency detection
- Optional dependencies
- Provider registration and resolution

#### 3. Controllers and Routing (NestJS: @Controller, @Get, @Post, etc.)
```rust
#[controller("/users")]
pub struct UserController;

#[impl_controller]
impl UserController {
    #[get("/")]
    pub async fn get_all_users(&self) -> Result<Vec<User>> {
        Ok(self.service.get_all())
    }

    #[post("/")]
    pub async fn create_user(
        #[body] user_dto: CreateUserDto
    ) -> Result<User, HttpException> {
        Ok(self.service.create(user_dto))
    }

    #[get("/:id")]
    pub async fn get_user(
        #[param("id")] id: Uuid
    ) -> Result<User, HttpException> {
        Ok(self.service.get_by_id(id))
    }
}
```
- Decorator-based routing
- Parameter extraction (body, param, query, headers)
- Type-safe route handlers
- Support for all HTTP methods
- Path parameters with typing

#### 4. Guards (NestJS: @UseGuards)
```rust
#[guard(AuthGuard)]
#[get("/protected")]
pub async fn protected_route() -> Result<String> {
    Ok("Protected content")
}

pub struct AuthGuard {
    #[inject]
    jwt_service: Arc<JwtService>,
}

impl Guard for AuthGuard {
    async fn can_activate(&self, context: &GuardContext) -> bool {
        self.jwt_service.validate(context).await
    }
}
```
- Route-level guards
- Multiple guards with AND/OR logic
- Access to request context
- Async guard execution

#### 5. Interceptors (NestJS: @UseInterceptors)
```rust
#[interceptor(LoggingInterceptor)]
#[interceptor(TimeoutInterceptor)]
#[get("/data")]
pub async fn get_data() -> Result<Data> {
    Ok(fetch_data())
}

pub struct LoggingInterceptor;

impl Interceptor for LoggingInterceptor {
    async fn intercept(&self, context: &InterceptorContext, next: Next) -> Result {
        log::info!("Request: {:?}", context.request);
        let result = next.call().await;
        log::info!("Response: {:?}", result);
        result
    }
}
```
- Pre/post request processing
- Response transformation
- Caching, logging, performance monitoring
- Async interceptor execution

#### 6. Pipes (NestJS: @UsePipes, built-in pipes)
```rust
#[pipe(ValidationPipe)]
#[post("/users")]
pub async fn create_user(
    #[body(ValidationPipe)] user: CreateUserDto
) -> Result<User> {
    Ok(user_service.create(user))
}

// Built-in pipes
pub struct ValidationPipe;

impl Pipe for ValidationPipe {
    type Input = T;
    type Output = T;

    fn transform(&self, value: Self::Input) -> Result<Self::Output, HttpException> {
        value.validate()?;
        Ok(value)
    }
}
```
- Data validation and transformation
- Built-in pipes: ValidationPipe, ParseIntPipe, ParseFloatPipe, etc.
- Custom pipes
- Chaining pipes

#### 7. Filters (NestJS: @Catch)
```rust
#[filter(HttpException)]
pub async fn http_exception_filter(
    exception: HttpException,
    context: ExceptionContext,
) -> HttpResponse {
    HttpResponse::BadRequest()
        .json(json!({
            "statusCode": exception.status,
            "message": exception.message
        }))
}

#[filter(NotFoundException)]
pub async fn not_found_filter(exception: NotFoundException) -> HttpResponse {
    HttpResponse::NotFound()
        .json(json!({
            "statusCode": 404,
            "message": "Resource not found"
        }))
}
```
- Global and scoped exception filters
- Custom exception types
- Exception context access
- Error response formatting

#### 8. Middleware (NestJS: NestMiddleware)
```rust
#[middleware]
pub struct LoggingMiddleware {
    #[inject]
    logger: Arc<Logger>,
}

#[impl_middleware]
impl Middleware for LoggingMiddleware {
    async fn use(&self, req: Request, res: Response, next: Next) -> Response {
        self.logger.log_request(&req);
        let response = next.run(req, res).await;
        self.logger.log_response(&response);
        response
    }
}

// Apply to module
#[module({
    middlewares: [LoggingMiddleware],
    // ...
})]
pub struct AppModule;
```
- Express-style middleware
- Module-level middleware
- Apply to routes or globally
- Async support

## Technical Implementation Strategy

### Phase 1: Core Foundation (Weeks 1-2)

#### 1.1 DI Container
- Create `nivasa` crate as main framework
- Implement `DependencyContainer` struct
- Support for `#[injectable]` attribute macro
- Provider registration and resolution
- Scope management (singleton, scoped, transient)
- Circular dependency detection
- Build: `nivasa-core` crate

#### 1.2 Module System
- Implement `Module` trait
- `#[module]` attribute macro for module configuration
- Module registry and dependency graph
- Module initialization lifecycle
- Import/export resolution
- Build: `nivasa-macros` crate for procedural macros

### Phase 2: Routing and Controllers (Weeks 3-4)

#### 2.1 Controller System
- `#[controller]` attribute macro
- `#[get]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]` attributes
- Route registration and matching
- Parameter extraction (body, param, query, headers)
- Response types (JSON, plain text, etc.)
- Build: `nest-rs-routing` crate

#### 2.2 HTTP Server Integration
- Use Hyper as HTTP backend (same as Axum)
- Server initialization and configuration
- Request/response handling
- WebSocket support (NestJS: @WebSocketGateway)
- Build: `nest-rs-http` crate

### Phase 3: Middleware and Guards (Weeks 5-6)

#### 3.1 Guard System
- `#[guard]` attribute macro
- `Guard` trait definition
- Guard execution pipeline
- Guard context (request, handler metadata)
- Multiple guards support

#### 3.2 Interceptor System
- `#[interceptor]` attribute macro
- `Interceptor` trait definition
- Interceptor chain execution
- Pre/post processing hooks
- Response transformation

#### 3.3 Middleware System
- `#[middleware]` attribute macro
- `Middleware` trait definition
- Middleware pipeline
- Apply to modules or routes

### Phase 4: Pipes and Validation (Weeks 7-8)

#### 4.1 Pipe System
- `#[pipe]` attribute macro
- `Pipe` trait definition
- Built-in pipes: ValidationPipe, ParseIntPipe, ParseFloatPipe, DefaultValuePipe
- Custom pipes
- Pipe chaining

#### 4.2 Validation Integration
- Integrate `validator` crate for DTO validation
- `@IsEmail`, `@IsString`, `@Min`, `@Max` decorators (via attribute macros)
- ValidationPipe implementation
- Error formatting

### Phase 5: Exception Handling (Weeks 9-10)

#### 5.1 Exception Filters
- `#[filter]` attribute macro
- `ExceptionFilter` trait definition
- Built-in filters: HttpExceptionFilter, NotFoundExceptionFilter
- Global and scoped filters
- Exception context

#### 5.2 Custom Exceptions
- Base `HttpException` type
- Built-in exceptions: BadRequestException, UnauthorizedException, NotFoundException, etc.
- Exception serialization

### Phase 6: Configuration and Testing (Weeks 11-12)

#### 6.1 Configuration Module
- Environment-based configuration
- `ConfigModule` with support for .env files
- Configuration service
- Type-safe configuration access

#### 6.2 Testing Utilities
- Test application builder
- Mock providers
- Integration testing helpers
- HTTP test client

#### 6.3 CLI Tool
- `nest-rs-cli` crate
- Project scaffolding: `nest-rs new project-name`
- Module generation: `nest-rs g module users`
- Controller generation: `nest-rs g controller users`
- Service generation: `nest-rs g service users`

### Phase 7: Advanced Features (Weeks 13-14)

#### 7.1 WebSocket Support
- `@WebSocketGateway` decorator
- `@SubscribeMessage`, `@MessageBody` decorators
- WebSocket adapter abstraction

#### 7.2 GraphQL Support (Optional)
- GraphQL module
- TypeDefs and Resolver decorators
- Federation support

#### 7.3 OpenAPI/Swagger Integration
- Automatic OpenAPI spec generation
- `@ApiTags`, `@ApiOperation` decorators
- Swagger UI integration

## Project Structure

```
nivasa/
├── nivasa/                    # Main framework crate
│   ├── src/
│   │   ├── module.rs
│   │   ├── controller.rs
│   │   ├── service.rs
│   │   └── lib.rs
├── nivasa-core/               # Core DI and utilities
│   ├── src/
│   │   ├── di/
│   │   │   ├── container.rs
│   │   │   ├── provider.rs
│   │   │   └── registry.rs
│   │   └── lib.rs
├── nivasa-macros/             # Procedural macros
│   ├── src/
│   │   ├── module.rs
│   │   ├── controller.rs
│   │   ├── injectable.rs
│   │   ├── guards.rs
│   │   ├── interceptors.rs
│   │   ├── pipes.rs
│   │   └── lib.rs
├── nivasa-http/               # HTTP server integration
│   ├── src/
│   │   ├── server.rs
│   │   ├── request.rs
│   │   ├── response.rs
│   │   └── lib.rs
├── nivasa-routing/            # Routing system
│   ├── src/
│   │   ├── router.rs
│   │   ├── route.rs
│   │   └── lib.rs
├── nivasa-guards/             # Guards
│   └── src/
│       ├── guard.rs
│       └── lib.rs
├── nivasa-interceptors/        # Interceptors
│   └── src/
│       ├── interceptor.rs
│       └── lib.rs
├── nivasa-pipes/              # Pipes
│   └── src/
│       ├── pipe.rs
│       ├── validation_pipe.rs
│       └── lib.rs
├── nivasa-filters/             # Exception filters
│   └── src/
│       ├── filter.rs
│       └── lib.rs
├── nivasa-validation/          # Validation integration
│   └── src/
│       ├── decorators.rs
│       └── lib.rs
├── nivasa-config/             # Configuration
│   └── src/
│       ├── config_module.rs
│       └── lib.rs
├── nivasa-websocket/          # WebSocket support
│   └── src/
│       ├── gateway.rs
│       └── lib.rs
├── nivasa-cli/                # CLI tool
│   ├── src/
│   │   ├── cli.rs
│   │   ├── generators/
│   │   └── lib.rs
├── nivasa-common/             # Common types
│   └── src/
│       ├── http_exception.rs
│       ├── dto.rs
│       └── lib.rs
├── examples/                   # Example applications
│   ├── basic/
│   ├── auth/
│   └── websocket/
├── tests/                     # Integration tests
│   └── ...
├── docs/                      # Documentation
│   └── ...
└── Cargo.toml
```

## Implementation Examples

### Complete Example: User Module

```rust
// dto/create_user_dto.rs
use nivasa::dto::Dto;
use nivasa_validation::IsEmail;
use nivasa_validation::Min;

#[derive(Deserialize, Dto)]
pub struct CreateUserDto {
    #[is_email]
    pub email: String,

    #[min(6)]
    pub password: String,

    pub name: String,
}

// entities/user.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
}

// services/user_service.rs
use nest_rs::service::Service;
use nest_rs::injectable;

#[injectable]
pub struct UserService {
    #[inject]
    repository: Arc<UserRepository>,
}

#[impl_service]
impl UserService {
    pub async fn create(&self, dto: CreateUserDto) -> Result<User> {
        let user = User {
            id: Uuid::new_v4(),
            email: dto.email,
            name: dto.name,
        };
        self.repository.save(user.clone()).await?;
        Ok(user)
    }

    pub async fn get_all(&self) -> Result<Vec<User>> {
        self.repository.find_all().await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<User> {
        self.repository.find_by_id(id).await
    }
}

// controllers/user_controller.rs
use nest_rs::controller::Controller;
use nest_rs::guards::Guard;
use nest_rs::HttpException;

#[controller("/users")]
pub struct UserController {
    #[inject]
    service: Arc<UserService>,
}

#[impl_controller]
impl UserController {
    #[get("/")]
    #[guard(AuthGuard)]
    pub async fn get_all_users(&self) -> Result<Vec<User>, HttpException> {
        Ok(self.service.get_all().await?)
    }

    #[post("/")]
    #[pipe(ValidationPipe)]
    pub async fn create_user(
        #[body] dto: CreateUserDto
    ) -> Result<User, HttpException> {
        Ok(self.service.create(dto).await?)
    }

    #[get("/:id")]
    pub async fn get_user(
        #[param("id")] id: Uuid
    ) -> Result<User, HttpException> {
        Ok(self.service.get_by_id(id).await?)
    }
}

// guards/auth_guard.rs
use nest_rs::guards::Guard;

pub struct AuthGuard {
    #[inject]
    jwt_service: Arc<JwtService>,
}

#[impl_guard]
impl Guard for AuthGuard {
    async fn can_activate(&self, context: &GuardContext) -> bool {
        let token = context.headers.get("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "));

        match token {
            Some(token) => self.jwt_service.validate(token).await.is_ok(),
            None => false,
        }
    }
}

// modules/user_module.rs
use nest_rs::module::Module;

#[module({
    imports: [],
    controllers: [UserController],
    providers: [UserService, UserRepository],
    exports: [UserService]
})]
pub struct UserModule;

// app.module.rs
use nest_rs::module::Module;
use nest_rs::http::HttpModule;

#[module({
    imports: [
        HttpModule,
        ConfigModule.forRoot(ConfigOptions {
            is_global: true,
            envFilePath: ".env",
        }),
        UserModule,
        AuthModule,
    ],
    controllers: [AppController],
    providers: [AppService],
})]
pub struct AppModule;

// main.rs
use nest_rs::NestApplication;
use nest_rs::http::ServerOptions;

#[tokio::main]
async fn main() -> Result<()> {
    let app = NestApplication::create(AppModule)
        .build()
        .await?;

    let server = ServerOptions {
        port: 3000,
        host: "0.0.0.0".to_string(),
        ..Default::default()
    };

    app.listen(server).await?;

    Ok(())
}
```

## Technical Decisions

### 1. Procedural Macros Over Macros
- Use `proc-macro` for all decorators
- Zero-cost abstractions at compile time
- Type-safe routing and DI

### 2. Hyper as HTTP Backend
- Use Hyper (same as Axum, Pavex)
- Excellent performance
- Async-first design
- Well-maintained

### 3. Tokio Runtime
- Use Tokio for async runtime
- Industry standard
- Great ecosystem

### 4. Serde for Serialization
- Use Serde for JSON/other formats
- Wide adoption
- Excellent performance

### 5. Integration with Existing Ecosystem
- Use Tower for middleware where possible
- Use Axum-compatible extractors
- Leverage existing validation crates

## Risks and Mitigations

### Risk 1: Procedural Macro Complexity
- Mitigation: Start simple, iterate incrementally
- Mitigation: Write extensive tests for macros
- Mitigation: Document macro behavior thoroughly

### Risk 2: Performance Overhead
- Mitigation: Compile-time DI (zero runtime cost)
- Mitigation: Benchmark against Actix/Axum
- Mitigation: Optimize hot paths

### Risk 3: Ecosystem Adoption
- Mitigation: Provide excellent documentation
- Mitigation: Create migration guides from NestJS
- Mitigation: Build example applications

### Risk 4: Scope Creep
- Mitigation: Focus on MVP first (DI + Routing + Controllers)
- Mitigation: Add advanced features in phases
- Mitigation: Follow NestJS feature parity

### Risk 5: Circular Dependencies in DI
- Mitigation: Implement cycle detection at compile time
- Mitigation: Provide clear error messages
- Mitigation: Support for optional/lazy dependencies

## Testing Strategy

### Unit Tests
- Test DI container resolution
- Test macro expansion
- Test guard/interceptor logic
- Test pipe transformations

### Integration Tests
- Test complete request flow
- Test module composition
- Test error handling
- Test authentication flows

### Benchmark Tests
- Compare with Actix/Axum
- Benchmark DI resolution
- Benchmark routing performance
- Benchmark startup time

## Success Criteria

1. **NestJS Feature Parity**: Support all major NestJS patterns (modules, DI, controllers, guards, interceptors, pipes, filters)

2. **Type Safety**: All decorators compile-time checked, no runtime reflection

3. **Performance**: Within 80% of Actix/Axum performance (DI should be zero-cost)

4. **Developer Experience**: Ergonomic API that feels natural to NestJS developers

5. **Documentation**: Complete guides and API documentation

6. **CLI Tool**: Scaffolding and code generation capabilities

7. **Testing**: >90% code coverage with comprehensive test suite

8. **Examples**: 5+ example applications demonstrating various features

## Next Steps

1. Create initial project structure
2. Implement basic DI container
3. Create procedural macro infrastructure
4. Build first working example (hello world)
5. Add controller and routing support
6. Implement module system
7. Add guards and interceptors
8. Create CLI tool
9. Write comprehensive documentation
10. Release v0.1.0

## Files Likely to Change

```
nest-rs/                    # New project
├── nest-rs-core/          # New crate
├── nest-rs-macros/        # New crate
├── nest-rs-http/          # New crate
├── nest-rs-routing/        # New crate
├── nest-rs-guards/         # New crate
├── nest-rs-interceptors/    # New crate
├── nest-rs-pipes/         # New crate
├── nest-rs-filters/        # New crate
├── nest-rs-validation/     # New crate
├── nest-rs-config/         # New crate
├── nest-rs-cli/           # New crate
└── examples/              # New directory
```

## Open Questions

1. Should we use an existing DI container or build our own?
   - Recommendation: Build custom DI to match NestJS patterns exactly

2. Should we support async/await in all hooks?
   - Recommendation: Yes, async-first design

3. Should we support GraphQL from day one?
   - Recommendation: No, defer to later phase

4. Should we provide database integration (TypeORM equivalent)?
   - Recommendation: Yes, provide adapters for popular ORMs (Diesel, SeaORM)

5. Should we implement a testing framework or use existing tools?
   - Recommendation: Use existing tools (axum-test, reqwest) with helper utilities
