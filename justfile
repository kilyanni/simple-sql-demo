[windows]
set shell := ["powershell.exe", "-NoLogo", "-Command"]

export DATABASE_URL := env("DATABASE_URL", if os_family() == "windows" {
    "postgres://postgres@localhost/moodle"
} else {
    "postgres://" + env("USER", "postgres") + "@localhost/moodle?host=/run/postgresql"
})
export TEST_DATABASE_URL := DATABASE_URL
export SQLX_OFFLINE := "true"
# Without this, psql on Windows reads the seed in the console codepage and garbles umlauts.
export PGCLIENTENCODING := "UTF8"

default:
    @just --list

# Create the database and init it with both schema and seed data
setup:
    cargo sqlx database create
    cargo sqlx migrate run
    psql "{{DATABASE_URL}}" -X -1 -v ON_ERROR_STOP=1 -f seeds/seed_large.sql

# Drop the database and set it up again
reset: && setup
    cargo sqlx database drop -y

run:
    cargo run

# Unit tests, then the query tests against the database
test:
    cargo test
    # Ideally these would be hidden behind a feature flag or something, but this is simpler at this scale.
    cargo test -- --ignored

# Refresh the compile-time query metadata after changing a query
prepare:
    SQLX_OFFLINE=false cargo sqlx prepare

fmt:
    cargo fmt
    python3 fmt_sql.py
    prettier -w README.md

lint:
    cargo clippy --all-targets
    cargo fmt --check

# Regenerate the demo data
seed:
    python3 seeds/gen_seed.py
