# Benchmark Notes

The benchmark harness currently covers one real workload:

- `hello_world_get_json_response` runs a loopback `GET /hello` request against a live Nivasa server and validates the JSON response body.

Run it locally with:

```bash
cargo bench -p nivasa-benchmarks --bench hello_world -- --quick --noplot
```

Current baseline:

- Harness: Criterion 0.5
- Transport: loopback HTTP server on `127.0.0.1`
- Workload: `GET /hello` returning `{"message":"hello world"}`
- Comparison targets: Actix Web and Axum rows are still open in `todo.md`

More benchmark rows are still open for DI resolution, routing scale, middleware pipeline overhead, and startup time.

Current local baseline from this branch:

- `hello_world_get_json_response`: `61.387 us` to `63.181 us` on a Criterion quick run

CI currently runs the benchmark target as a smoke surface. A stricter baseline comparator for regression detection is still open in `todo.md`.
