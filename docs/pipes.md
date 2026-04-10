# Pipes

This page documents the pipe surface that is already landed in `nivasa-pipes` and the current validation-pipe boundary.

## SCXML Rule

Keep pipe execution inside the request pipeline. Pipes transform or validate argument values after extractor metadata is resolved, but they do not define a separate lifecycle outside the SCXML-backed request flow.

## What Ships Today

`nivasa-pipes` currently exposes these building blocks:

1. `Pipe` as the value-transform trait.
1. `ArgumentMetadata` as the per-argument metadata bundle.
1. `PipeChain` for sequencing multiple pipes.
1. `TrimPipe` for whitespace trimming.
1. `DefaultValuePipe` for replacing explicit `null` with a default value.
1. `ParseBoolPipe` for boolean coercion.
1. `ParseIntPipe<T>` for integer coercion.
1. `ParseFloatPipe<T>` for float coercion.
1. `ParseUuidPipe` for UUID coercion.
1. `ParseEnumPipe<T>` for enum-like coercion.
1. `ValidationPipe<T>` for deserialize-then-validate DTOs.

The pipe trait is intentionally small: it accepts a `serde_json::Value` plus `ArgumentMetadata`, then returns either a transformed value or an `HttpException`.

## Built-In Pipes

The built-in pipes are narrow and predictable:

1. `TrimPipe` trims outer whitespace from string values.
1. `DefaultValuePipe` substitutes a configured value when the input is explicit `null`.
1. `ParseBoolPipe` accepts string inputs such as `"true"` and `"false"`.
1. `ParseIntPipe<T>` supports `i32` and `i64`.
1. `ParseFloatPipe<T>` supports `f32` and `f64`.
1. `ParseUuidPipe` accepts UUID text and normalizes it to canonical string form.
1. `ParseEnumPipe<T>` delegates parsing to the target type and converts the result back into JSON.
1. `ValidationPipe<T>` deserializes a JSON object into `T`, runs `Validate`, and returns structured `HttpException` details when validation fails.

See:

- [`/Users/ektasaini/Desktop/nivasa/nivasa-pipes/src/lib.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-pipes/src/lib.rs)
- [`/Users/ektasaini/Desktop/nivasa/nivasa-pipes/src/lib.rs#L412`](/Users/ektasaini/Desktop/nivasa/nivasa-pipes/src/lib.rs)

## Custom Pipes

Custom pipes are just user types that implement `Pipe`:

1. Build a type that implements `Pipe::transform(...)`.
1. Read the incoming JSON value plus `ArgumentMetadata`.
1. Return a transformed value or an `HttpException`.

That makes custom pipes easy to test and easy to chain with `PipeChain`.

The landed `ArgumentMetadata` fields are:

1. `param_name`
1. `metatype`
1. `data_type`
1. `index`

The metadata is there so later extractor and decorator wiring can tell a pipe what kind of argument it is transforming.

## Validation Boundary

`ValidationPipe<T>` is the most opinionated built-in pipe today, but it still has a clear boundary:

1. It expects a JSON object value.
1. It deserializes into `T` with `serde`.
1. It runs `Validate` from `nivasa-validation`.
1. It returns a `400` `HttpException` with structured validation details on failure.

What it does not do yet:

1. It does not become a full controller-execution engine by itself.
1. It does not auto-run on every request without metadata wiring.
1. It does not replace extractor or route logic.

## Proof Points

The current behavior is covered by focused tests in [`nivasa-pipes/src/lib.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-pipes/src/lib.rs), including:

1. integer, float, boolean, UUID, enum, and trim coercion tests
1. default-value behavior
1. validation success and validation failure tests
1. `PipeChain` sequencing

## Practical Notes

1. Use built-in pipes for common coercion and validation.
1. Use custom `Pipe` implementations when you need domain-specific transforms.
1. Treat `ValidationPipe` as a narrow DTO validator, not as a broader request pipeline replacement.
