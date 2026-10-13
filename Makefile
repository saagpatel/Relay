.PHONY: build test lint clean check run

build:
	cargo build --manifest-path client/src-tauri/Cargo.toml --release --locked

check:
	@command -v go >/dev/null 2>&1 || { echo "missing Go; install the version pinned in server/go.mod" >&2; exit 127; }
	@command -v gofmt >/dev/null 2>&1 || { echo "missing gofmt; install the version pinned in server/go.mod" >&2; exit 127; }
	@command -v cargo >/dev/null 2>&1 || { echo "missing Cargo; install the version pinned in rust-toolchain.toml" >&2; exit 127; }
	@command -v rustc >/dev/null 2>&1 || { echo "missing rustc; install the version pinned in rust-toolchain.toml" >&2; exit 127; }
	@command -v rustfmt >/dev/null 2>&1 || { echo "missing rustfmt; install the rustfmt component from rust-toolchain.toml" >&2; exit 127; }
	@command -v clippy-driver >/dev/null 2>&1 || { echo "missing Clippy; install the clippy component from rust-toolchain.toml" >&2; exit 127; }
	@command -v pnpm >/dev/null 2>&1 || { echo "missing pnpm; enable Corepack or install the version pinned in client/package.json" >&2; exit 127; }
	@command -v node >/dev/null 2>&1 || { echo "missing Node.js; install the version pinned in .node-version" >&2; exit 127; }
	@set -eu; unformatted="$$(gofmt -l server)"; if [ -n "$$unformatted" ]; then printf 'Go files need gofmt:\n%s\n' "$$unformatted" >&2; exit 1; fi
	cd server && go mod verify
	cd server && go vet ./...
	cd server && go test -race ./...
	cargo fmt --manifest-path client/src-tauri/Cargo.toml --all -- --check
	cargo clippy --manifest-path client/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
	cargo test --manifest-path client/src-tauri/Cargo.toml --locked -- --test-threads=1
	node --test client/src/lib/phase-transition.test.mjs
	cd client && pnpm install --frozen-lockfile
	cd client && pnpm typecheck
	cd client && pnpm build

test:
	cargo test --manifest-path client/src-tauri/Cargo.toml --locked -- --test-threads=1

lint:
	cargo clippy --manifest-path client/src-tauri/Cargo.toml --locked --all-targets -- -D warnings

run:
	cargo run --manifest-path client/src-tauri/Cargo.toml

clean:
	cargo clean --manifest-path client/src-tauri/Cargo.toml
