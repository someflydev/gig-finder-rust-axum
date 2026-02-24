set shell := ["/bin/sh", "-cu"]

default:
    @just --list

dev:
    @echo "Starting RHOF development flow"
    just db-up
    just serve

db-up:
    docker compose up -d postgres

db-down:
    docker compose down

migrate:
    cargo run -p rhof-cli -- migrate

sqlx-prepare:
    cargo sqlx prepare --workspace

tailwind-install:
    sh scripts/install-tailwind.sh

sync:
    cargo run -p rhof-cli -- sync

serve:
    cargo run -p rhof-cli -- serve

test:
    cargo test --workspace

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

tailwind:
    ./bin/tailwindcss -i assets/tailwind/input.css -o assets/static/app.css --config assets/tailwind/tailwind.config.js
