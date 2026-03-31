# Contributing

Thanks for your interest in improving MEV Protocol.

## Development Workflow

1. Create a feature branch from `main`: `git checkout -b feat/my-change`
2. Keep changes scoped and focused — one concern per PR.
3. Run local quality gates before opening a PR.
4. Open a PR using the provided template.

## Prerequisites

| Tool | Version | Check |
|------|---------|-------|
| Rust | 1.75+ | `rustc --version` |
| Go | 1.21+ | `go version` |
| Foundry | Latest | `forge --version` |
| GCC/Clang | 11+ | `gcc --version` |

## Local Quality Gates

From repository root:

```bash
make build
make test
make lint
```

Stack-specific checks:

```bash
# Rust
cd core && cargo fmt --all -- --check
cd core && cargo clippy --all-targets --all-features -- -D warnings
cd core && cargo test

# Go
cd network && go test ./... -v
cd network && go vet ./...

# Solidity
cd contracts && forge fmt --check
cd contracts && forge test -vvv

# C
cd fast && make test
```

## Code Style

### Rust
- **Formatter:** `rustfmt` with default config. Run `cargo fmt --all` before committing.
- **Linter:** `cargo clippy -D warnings` — zero warnings policy.
- **Naming:** `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE` for constants.
- **Error handling:** Use `anyhow::Result` for application errors, `thiserror` for library errors. Avoid `.unwrap()` in non-test code.
- **Doc comments:** All public items must have `///` doc comments. Modules need `//!` headers.

### Go
- **Formatter:** `gofmt` (enforced). Run `gofmt -w .` before committing.
- **Linter:** `go vet ./...` must pass.
- **Naming:** Follow [Effective Go](https://go.dev/doc/effective_go) conventions.
- **Doc comments:** All exported functions and types must have godoc comments starting with the identifier name.

### Solidity
- **Formatter:** `forge fmt` with default config.
- **NatSpec:** All public/external functions must have `@notice`, `@param`, and `@return` tags. Contracts need `@title` and `@author`.
- **Custom errors:** Prefer `error MyError()` over `require(cond, "string")` for gas efficiency.

### C
- **Style:** K&R braces, 4-space indentation.
- **Headers:** Every public function in `include/*.h` must have a doc comment describing parameters, return value, and thread safety.
- **Memory:** All allocations go through the arena pool (`memory_pool.c`). No raw `malloc` in hot paths.

## Commit Messages

Use the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>

[optional body]
```

**Types:** `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `ci`, `chore`

**Scopes:** `core`, `network`, `contracts`, `fast`, `config`, `docker`, `proto`

**Examples:**
```
feat(core): add liquidation detection for Aave V3
fix(network): prevent panic on uint64 overflow in mempool monitor
perf(fast): add AVX2 batch address comparison
test(contracts): add fork-mode integration test for flash arbitrage
docs(config): add CONFIG.md with field descriptions
```

## Pull Request Guidelines

### Before Opening

- [ ] All quality gates pass locally (`make build test lint`)
- [ ] New code has tests (unit, integration, or property-based as appropriate)
- [ ] Benchmark evidence included for performance-sensitive changes (`cargo bench` before/after)
- [ ] Documentation updated if behaviour or interfaces changed
- [ ] No secrets, credentials, or API keys in the diff

### PR Description Template

```markdown
## What
One-sentence summary of the change.

## Why
Context: what problem does this solve?

## How
Brief technical approach.

## Test Evidence
Paste test output or benchmark comparison.

## Operational Impact
Does this change config, deployment steps, or resource requirements?
```

### Review Expectations

- PRs should be **small enough to review in one sitting** (< 400 lines preferred).
- Reviewers check correctness, test coverage, and style compliance.
- Address all review comments before merging — resolve threads explicitly.

## What Makes a Good PR

- **Correctness fix** — with a regression test proving the bug existed
- **Performance improvement** — with before/after Criterion benchmarks
- **Observability improvement** — new metrics, better log messages, dashboards
- **Documentation alignment** — keeping docs in sync with implementation

## Security

Do not commit real secrets, credentials, or private keys.
Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
