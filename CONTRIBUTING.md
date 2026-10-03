# Contributing

Thanks for your interest in contributing! Here's how to get started.

## Bug Reports & Feature Requests

Open a [GitHub Issue](../../issues/new) with:
- Clear description of the problem or idea
- Steps to reproduce (for bugs)
- Expected vs actual behavior

## Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/your-feature`)
3. Make your changes with clear commit messages
4. Run existing tests to ensure nothing breaks
5. Open a PR with a description of what changed and why

## Development Setup

See the README for installation and setup instructions.

## Code Style

- Follow the existing patterns in the codebase
- Use meaningful variable and function names
- Add comments only where the logic isn't self-evident

## Questions?

Open an issue or start a discussion. Response time is typically within a few days.

## Verification

Start at the repository root. The project has three separate lanes: Go in
`server/`, Rust/Tauri in `client/src-tauri/`, and the frontend in `client/`.
Use the toolchains and native OS packages in
[test-build-release.yml](.github/workflows/test-build-release.yml) (Go 1.25.13,
Node 22.12+, pnpm 10, stable Rust). Linux also needs GTK/WebKit Tauri prerequisites.

Focused offline configuration coverage:

```sh
(cd server && go test -run '^TestLoadConfigFromEnv' ./...)
```

Broader correctness and frontend checks:

```sh
(cd server && go test -race ./...)
cargo test --manifest-path client/src-tauri/Cargo.toml --lib --bins
(cd client && pnpm install --frozen-lockfile)
(cd client && pnpm typecheck)
(cd client && pnpm build)
```

Rust tests can be narrowed with a test-name filter. Root Makefile targets cover
Rust only. For changed Rust files, check formatting with
`cargo fmt --manifest-path client/src-tauri/Cargo.toml --all -- --check` and lint
with `cargo clippy --manifest-path client/src-tauri/Cargo.toml --all-targets -- -D warnings`.
There is no frontend unit-test script in `client/package.json`; typecheck/build
must not be reported as frontend tests.

Transport changes also need the existing integration target:

```sh
cargo test --manifest-path client/src-tauri/Cargo.toml --test signaling_e2e -- --test-threads=1
```

Inspect its preconditions before execution: it rebuilds `server/relay-server`,
starts a test-owned loopback signaling process and QUIC endpoints, and uses
synthetic temporary files. Some cases print `SKIP` when Go/server binaries are
unavailable; a successful exit alone does not prove that they ran. Use an
isolated checkout for this lane, and retain skipped-case output.

The [managed verification contract](.codex/verify.commands) and its
[runner](.codex/scripts/run_verify_commands.sh) also include vulnerability scans,
tool installations, performance outputs, and release-evidence validation.
Inspect those requirements before running the full contract; offline checks
above do not satisfy those separate gates. Do not change scan exceptions or
release credentials to make a local check pass.

For UI/transfer changes, use [LOCAL_SMOKE.md](docs/LOCAL_SMOKE.md) with disposable
files and a test-owned server. Browser preview (`cd client && pnpm dev`) covers
appearance only; native IPC and real direct/fallback behavior need desktop
validation. Documentation-only changes do not require application launch,
network transfers, or public release validation.
