# Contributing

Thanks for your interest in improving MEV Protocol.

## Development workflow

1. Create a feature branch from `main`.
2. Keep changes scoped and focused.
3. Run local quality gates before opening a PR.
4. Open a PR using the provided template.

## Local quality gates

From repository root:

```bash
make build
make test
make lint
```

If needed, run stack-specific checks:

- `cd core && cargo fmt --all -- --check`
- `cd core && cargo clippy --all-targets --all-features -- -D warnings`
- `cd network && go test ./...`
- `cd contracts && forge test`

## Commit and PR guidelines

- Use clear, imperative commit messages.
- Keep PRs small enough to review effectively.
- Include test evidence and operational impact in PR description.
- Update docs when behavior or interface changes.

## Scope expectations

Good PRs typically include one of:

- correctness fix
- performance improvement
- observability/devex improvement
- docs alignment with implementation

## Security

Do not commit real secrets, credentials, or private keys.
Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
