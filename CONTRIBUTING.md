# Contributing

## Building

```bash
cargo build --workspace
```

## Running tests

```bash
# All unit tests
cargo test --workspace

# Individual crates
cargo test -p go-model
cargo test -p go-analyzer --lib

# Integration tests
cargo test -p go-analyzer --test fluent_api_test
cargo test -p go-analyzer --test rewrite_e2e_test
cargo test -p go-analyzer --test printer_roundtrip
cargo test -p go-analyzer --test formatting_test
cargo test -p go-analyzer --test regression_test
```

## Corpus test

The corpus test roundtrips every `.go` file in the Go standard library through the walker and printer. It requires Go to be installed:

```bash
cargo test -p go-analyzer --test corpus_test -- --nocapture
```

## Code style

Format and lint before submitting:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

All warnings are treated as errors in CI.

## PR process

1. Fork and create a feature branch.
2. Make your changes. Add tests for new behavior.
3. Run `cargo fmt --all`, `cargo clippy --all-targets`, and `cargo test --workspace`.
4. Open a pull request against `master`. CI runs automatically.
