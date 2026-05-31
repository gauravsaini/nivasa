# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Nivasa, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

### How to Report

1. **Email:** Send a detailed report to **security@nivasa.dev**
2. **Subject line:** `[SECURITY] Brief description of the issue`
3. **Include:**
   - A clear description of the vulnerability
   - Steps to reproduce or a proof-of-concept
   - The affected crate(s) and version(s)
   - Any potential impact assessment
   - Suggested fix (if you have one)

### What to Expect

- **Acknowledgement:** We will acknowledge receipt within **48 hours**.
- **Assessment:** We will provide an initial assessment within **7 days**.
- **Fix timeline:** Critical vulnerabilities will be patched within **14 days**.
  Lower-severity issues will be addressed in the next scheduled release.
- **Disclosure:** We will coordinate with you on public disclosure timing.
  We follow a 90-day disclosure deadline from initial report.

### Recognition

We appreciate the security research community's efforts. Reporters who follow
responsible disclosure will be credited in the release notes (unless they prefer
to remain anonymous).

## Security Best Practices for Users

When using Nivasa in production:

- Keep dependencies up to date (`cargo update` regularly)
- Run `cargo audit` and `cargo deny check advisories` in CI
- Enable TLS for all production deployments
- Use the `AuthGuard` and `RolesGuard` for access control
- Configure rate limiting via `ThrottlerModule` to prevent abuse
- Set appropriate `request_body_size_limit` and `request_timeout` values
- Review CORS configuration — avoid permissive defaults in production
- Use `ConfigModule` with schema validation to catch misconfiguration at startup

## Dependency Security

This project uses:

- **`cargo-deny`** to audit licenses and known vulnerabilities in CI
- **`cargo audit`** advisories checks in the release workflow
- **Dependabot** (when enabled) for automated dependency update PRs
- **Minimal dependency policy** — we avoid unnecessary transitive dependencies
