# Security policy

AI Agent Access Inspector reads configuration files of AI agents on the user's machine, some of which may contain sensitive data. We take security issues seriously and appreciate responsible disclosure.

## Supported versions

The project is in early development and has no stable release yet. Security fixes are made on the `main` branch and included in the next release. Once stable releases exist, this section will list the supported versions.

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues, discussions or pull requests.**

Report them privately through GitHub's private vulnerability reporting:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Describe the issue, how to reproduce it and its possible impact.

What to expect:

- an acknowledgement within 7 days,
- an assessment and, if confirmed, a plan for a fix,
- credit in the release notes, unless you prefer to stay anonymous.

Please give us reasonable time to fix the issue before disclosing it publicly.

## Scope

Relevant are, for example:

- secrets (tokens, API keys, account data, environment values of MCP servers) appearing in scan output or exports,
- crashes, hangs or excessive resource use caused by crafted configuration files,
- the application writing to or changing agent configuration, which it must never do,
- the application sending data over the network, which it must never do,
- weaknesses in the release pipeline or the published artifacts.

Not in scope:

- findings that are reported correctly but are about the scanned agent's own security (please report those to the agent's vendor),
- limitations already documented in [`docs/claude-code-scanner.md`](docs/claude-code-scanner.md) (section "Known limitations").

## How the project handles security

- Dependencies are checked for known vulnerabilities, licenses and sources with `cargo deny` on every CI run and weekly; Dependabot proposes updates.
- Each development phase includes a security check, and a full security review takes place before the first public release (see [`docs/roadmap.md`](docs/roadmap.md), section 14).
