# Security Policy

Agate is a security gateway, so we take vulnerabilities seriously.

## Reporting a vulnerability

**Do not open a public issue for security problems.**

Use GitHub's private vulnerability reporting:
**Security → Advisories → "Report a vulnerability"**
(`https://github.com/C3EQUALZz/agate/security/advisories/new`).

Please include:

- a description and impact assessment,
- reproduction steps or a proof of concept,
- affected version / commit,
- any suggested remediation.

We aim to acknowledge reports within a few days and to coordinate a fix and
disclosure timeline with you.

## Supported versions

The project is pre-1.0 and under active development; only the latest `main` is
supported. Pin a commit if you need stability.

## Scope notes

Agate enforces policy and produces a tamper-evident audit log. Reports about the
integrity of the transparency log (forgery, split-view, signature/verification
bypass), policy-enforcement bypasses, and Wasm plugin sandbox escapes are
especially in scope.
