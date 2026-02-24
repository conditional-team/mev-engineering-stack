# Security Policy

## Supported use

This repository is intended for engineering research and education.

## Reporting a vulnerability

Please do not open public issues for undisclosed vulnerabilities.

Instead, report privately to project maintainers with:

- affected component and path
- impact summary
- reproduction steps
- suggested mitigation (if available)

## Secret handling

- Never commit private keys, tokens, RPC credentials, or signed payloads.
- Use `config/.env.example` as template only.
- Keep runtime secrets in local `.env` files excluded by `.gitignore`.

## Hardening baseline

Before releases and major PRs:

1. Run build/test/lint gates.
2. Re-check config defaults for unsafe values.
3. Verify no sensitive values exist in docs, scripts, or examples.
4. Validate deployment scripts against target chain and account scopes.
