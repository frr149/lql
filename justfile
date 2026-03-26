# lql — Linear Query Language

# Compilar en modo debug
build:
    cargo build

# Tests unitarios (270, sin API)
test:
    cargo test

# Tests de integración contra Linear API real
integration:
    cargo test -- --ignored

# Todos los tests (unit + integration)
all:
    cargo test -- --include-ignored

# Compilar release nativo
release:
    cargo build --release

# Cross-compile para wuwei (Linux musl)
cross:
    cargo build --release --target x86_64-unknown-linux-musl

# Deploy a wuwei
deploy: cross
    scp target/x86_64-unknown-linux-musl/release/lql wuwei.frr.dev:~/.local/bin/

# Lint (warnings = error)
lint:
    cargo clippy -- -D warnings

# Ejecutar un comando lql en modo debug
run *ARGS:
    cargo run -- {{ARGS}}
