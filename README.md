# app-infra

Shared Rust infrastructure crates for application services. This workspace was extracted from `milon-indexer` so its
configuration, persistence, web, and tracing foundations can be developed and tested independently.

## Workspace crates

| Crate | Purpose |
| --- | --- |
| `infra-core` | Configuration, application errors and results, runtime pools, retry utilities, and UUID helpers. |
| `infra-rdb-cfg` | Configuration types for the RocksDB infrastructure. |
| `infra-rdb` | Typed RocksDB access, codecs, durable batches, iterators, and TTL support. |
| `axum-resp-macro` | Procedural macros used by the Axum response layer. |
| `infra-web` | Axum middleware and HTTP response helpers. |
| `infra-tracing` | Tracing initialization and rolling log appenders. |

## Development

The workspace uses the pinned `nightly-2026-05-26` Rust toolchain. Install it with Rustup if it is not already
available:

```bash
rustup toolchain install nightly-2026-05-26
```

Run the standard checks from this directory:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Integration with milon-indexer

`milon-indexer` consumes these packages through sibling path dependencies such as
`../app-infra/infra-core`. Package names and public APIs are kept stable, so application crates can continue to
depend on them through the indexer's workspace dependency catalog.
