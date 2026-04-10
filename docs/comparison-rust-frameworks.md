# Comparison with Other Rust Frameworks

Nivasa is not trying to be a generic Rust web framework clone. It is a
NestJS-style framework with modules, decorators, SCXML-backed lifecycles, and
bootstrap surfaces that stay close to the NestJS mental model.

This page gives a high-level comparison with common Rust frameworks. It is
meant to help you place Nivasa, not to rank winners.

## The Short Version

1. **Actix Web** is a mature, fast, low-level web framework with a broad
   ecosystem and very explicit control.
1. **Axum** is a composable, tower-based framework with a strong extractor
   story and excellent ergonomics for service composition.
1. **Poem** leans into a more integrated developer experience with routing,
   OpenAPI, and middleware in one package.
1. **Rocket** emphasizes an expressive, batteries-included macro-driven style.
1. **Pavex** takes a compile-time, architecture-first approach with a strong
   focus on correctness and generated code.
1. **Nivasa** focuses on NestJS pattern compatibility: modules, DI, decorators,
   guards, interceptors, pipes, filters, and SCXML-defined lifecycle control.

## Where Nivasa Fits

Nivasa is best understood as a framework for teams that want NestJS-like
structure in Rust.

Current strengths:

- module composition with imports, exports, and lifecycle hooks
- dependency injection with singleton, scoped, and transient providers
- decorator-driven controllers and parameter extraction
- guard, interceptor, pipe, filter, and WebSocket surfaces
- SCXML-backed lifecycle enforcement so request and provider transitions stay
  explicit

If you want a compact mental model, Nivasa is closer to "NestJS patterns in
Rust" than to "another Axum clone."

## Compared With Actix Web

Actix Web is a strong choice when you want direct control and a highly mature
HTTP stack.

Compared with Actix Web, Nivasa offers:

- a module system instead of a mostly manual app composition style
- DI and lifecycle hooks as first-class patterns
- decorator-style controller metadata instead of route wiring by hand

Compared with Nivasa, Actix Web offers:

- a larger ecosystem and a longer production history
- lower-level control over request handling and middleware composition
- a more established performance reputation

## Compared With Axum

Axum is a great fit when you like tower composition, extractors, and explicit
middleware stacks.

Compared with Axum, Nivasa offers:

- NestJS-like modules and provider visibility
- decorators for controllers, guards, interceptors, and pipes
- SCXML-enforced lifecycle steps around the request path

Compared with Nivasa, Axum offers:

- a simpler path if you already think in tower services and extractors
- a very mature and widely used ecosystem
- a more direct fit for service-oriented Rust web architecture

## Compared With Poem

Poem and Nivasa both aim for a higher-level developer experience than a bare
HTTP stack.

Compared with Poem, Nivasa offers:

- stronger NestJS-like structure around modules and providers
- SCXML-defined lifecycle contracts
- decorator-driven architecture that tracks the NestJS vocabulary more closely

Compared with Nivasa, Poem offers:

- a more established all-in-one web framework experience
- a simpler story if you want a framework that feels complete without a lot of
  architectural ceremony

## Compared With Rocket

Rocket is highly ergonomic and macro-driven, with a very opinionated developer
experience.

Compared with Rocket, Nivasa offers:

- modules and DI as core architectural primitives
- explicit provider lifecycle control
- controller/guard/interceptor/pipe/filter surfaces that map more directly to
  NestJS concepts

Compared with Nivasa, Rocket offers:

- a more mature macro-driven ergonomics story
- a simpler path if you prefer Rocket's single-framework style over a modular
  application container

## Compared With Pavex

Pavex and Nivasa both care about structure and correctness, but they approach
the problem differently.

Compared with Pavex, Nivasa offers:

- NestJS-style decorators and module vocabulary
- SCXML lifecycle definitions that are visible in the repository today
- a more direct alignment with application patterns from NestJS

Compared with Nivasa, Pavex offers:

- a more compiler-centric architecture pipeline
- a design that is especially focused on generated application blueprints

## Honest Take

Nivasa is a good fit when:

- you want NestJS-style architecture in Rust
- you value modules, DI, and decorator-based structure over a minimal routing API
- you want lifecycle behavior to be explicit and checked through SCXML

It is not the best fit when:

- you want the most battle-tested mainstream Rust web stack today
- you want minimal framework ceremony
- you want to avoid an opinionated architecture

## Practical Advice

1. Pick Nivasa if the framework shape itself is part of the product.
1. Pick Actix Web or Axum if you want a more mainstream Rust web core.
1. Pick Poem or Rocket if you want a more integrated macro-heavy framework.
1. Compare Nivasa against your architecture goals, not just raw framework
   ergonomics.
