# Chronos sandbox CI lanes

# Run smoke tests (no real containers, <5 min)
sandbox-smoke:
    cargo test -p chronos-sandbox -- --test-threads=1

# Run integration tests for a specific language (requires podman)
sandbox-java:
    cargo test -p chronos-sandbox --test test_java -- --include-ignored --test-threads=1

sandbox-rust:
    cargo test -p chronos-sandbox --test test_rust -- --include-ignored --test-threads=1

sandbox-go:
    cargo test -p chronos-sandbox --test test_go -- --include-ignored --test-threads=1

sandbox-python:
    cargo test -p chronos-sandbox --test test_python -- --include-ignored --test-threads=1

sandbox-nodejs:
    cargo test -p chronos-sandbox --test test_nodejs -- --include-ignored --test-threads=1

# Run all integration tests (requires podman)
sandbox-full:
    cargo test -p chronos-sandbox -- --include-ignored --test-threads=1

# Run benchmarks
sandbox-bench:
    cargo bench -p chronos-sandbox

# Build sandbox only
sandbox-build:
    cargo build -p chronos-sandbox

# Check all (build + clippy + test smoke)
sandbox-check:
    cargo build -p chronos-sandbox
    cargo clippy -p chronos-sandbox -- -D warnings
    cargo test -p chronos-sandbox -- --test-threads=1

# Pull/refresh container images for sandbox targets
sandbox-pull:
    podman pull eclipse-temurin:21-jdk
    podman pull golang:1.22
    podman pull python:3.12
    podman pull node:20
