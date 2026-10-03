.PHONY: build test lint clean check run

build:
	cargo build --manifest-path client/src-tauri/Cargo.toml --release

check:
	cargo check --manifest-path client/src-tauri/Cargo.toml

test:
	cargo test --manifest-path client/src-tauri/Cargo.toml -- --test-threads=1

lint:
	cargo clippy --manifest-path client/src-tauri/Cargo.toml -- -D warnings

run:
	cargo run --manifest-path client/src-tauri/Cargo.toml

clean:
	cargo clean --manifest-path client/src-tauri/Cargo.toml
