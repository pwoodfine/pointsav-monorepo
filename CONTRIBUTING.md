# Contributing

Version 1.0 — Effective 2026-04-20
Copyright (c) 2026 Woodfine Capital Projects Inc.

This document describes how to contribute to repositories managed through
factory-release-engineering. It is incorporated by reference into the
license terms of every AGPLv3, FSL-1.1, and Apache-2.0 repository.

---

## 1. Before you contribute

Read `CODE_OF_CONDUCT.md`. By contributing, You agree to its terms.

Check whether the repository You're contributing to requires a signed
Contributor License Agreement (CLA). This is described in Section 7.

For significant changes, open a discussion issue first. This saves You
writing a PR that the maintainers cannot accept for reasons of scope,
architecture, or direction.

## 2. Filing issues

Use the issue templates in `.github/ISSUE_TEMPLATE/`:

  - **Bug reports** — describe what happened, what You expected, and
    how to reproduce it.
  - **Feature requests** — describe the use case and proposed approach.

Do **not** file security vulnerabilities as public issues. See
`SECURITY.md` for the disclosure channel.

Maintainers aim to acknowledge issues within five (5) business days,
consistent with the response posture in `SECURITY.md`. Triage and fix
timelines are not committed for non-security issues.

## 3. Proposing changes

Standard fork-and-pull-request workflow:

  1. Fork the repository.
  2. Create a branch for Your change. Branch names must use one of the
     following prefixes: `feature/`, `fix/`, `docs/`, or `chore/`.
  3. Make Your changes in small, focused commits. Commit messages must
     follow the Conventional Commits convention (`feat:`, `fix:`,
     `docs:`, `refactor:`, `chore:`, `test:`, etc.).
  4. Push Your branch and open a PR against `main`.
  5. Address review feedback.
  6. A maintainer merges once the PR is approved and CI passes.

## 4. Code style and quality

Each repository's source tree includes an `.editorconfig` file for
baseline formatting (indentation, line endings, final newline). Honor
it.

Language-specific linters and formatters are configured per-repository
and enforced in CI. Run them locally before opening a PR to avoid
round-trips.

Per-repository style rules beyond `.editorconfig` and linter config —
where any exist — live in that repository's own `CONTRIBUTING.md` or
style-guide documents. Cross-repository baseline rules, if any are
ever introduced, would live in this file.

## 5. Testing

All PRs that change behavior must include tests for the new or changed
behavior. All PRs must pass CI before merge. No absolute test-coverage
threshold is enforced at the factory-release-engineering level;
individual repositories may set thresholds in their own CI
configuration.

## 6. Review process

Every PR requires at least one approving review from a maintainer
listed in `CODEOWNERS` before merge. CI must be green. No mandatory
second reviewer is required at the factory-release-engineering level.
Individual repositories may require a second approval for specific
file paths via their `CODEOWNERS` (e.g., `LICENSE`, `CONTRIBUTING.md`,
auth-related code).

Maintainers may request changes, approve, or close the PR with
explanation. Maintainers merge using "squash and merge" unless a
different strategy is documented in the repository.

## 7. Contributor License Agreement (CLA)

### When a CLA is required

A signed CLA is required before Your first merge to any repository
licensed under **AGPL-3.0-or-later** or **FSL-1.1-ALv2**. The CLA is
administered via CLA Assistant, which checks for a signature
automatically on every PR.

No CLA is required for repositories licensed under Apache-2.0, CC BY
4.0, CC BY-ND 4.0, or PointSav-ARR. For those repositories, a
Developer Certificate of Origin (DCO) sign-off in the commit message
is sufficient (the `Signed-off-by:` line added by `git commit -s`).

### Individual vs. corporate

  - **Individual CLA** — if You are contributing on Your own behalf.
  - **Corporate CLA** — if You are contributing as part of Your
    employment or under a contract where Your employer claims rights
    in Your contributions. The corporate CLA is signed by an
    authorized signatory of Your employer and covers all employees
    contributing on behalf of that employer.

The CLA forms live in `factory-release-engineering/cla/` and are
linked from the CLA Assistant prompt that appears on Your first PR to
a CLA-requiring repository.

### What the CLA is not

The CLA is not a copyright assignment. You retain the copyright in
Your contributions. The CLA is a license from You to Woodfine Capital
Projects Inc. to use, relicense, and distribute Your contributions
under the terms of the repository's license and any commercial
alternative license Woodfine offers.

## 8. Security

Do not file security vulnerabilities as public issues or PRs. See
`SECURITY.md` for the disclosure channel.

## 9. Code of Conduct

By contributing, You agree to abide by `CODE_OF_CONDUCT.md`.
Violations may result in warning, temporary ban, or permanent ban per
the enforcement ladder in that document.

## 10. License grant (repositories without a CLA)

For repositories not requiring a CLA (see Section 7), You represent
that Your contribution is Your own work or that You have the right to
contribute it under the repository's license, and You agree that Your
contribution is licensed to the project under that license. A
`Signed-off-by` DCO line in Your commit message records this
agreement.

## 11. Governance of this document

This `CONTRIBUTING.md` is maintained in
`factory-release-engineering/policies/`. Changes follow the governance
described in that directory's README §5. Repository-specific additions
may live in `CONTRIBUTING.md` files within individual repositories and
are merged with this document at propagation time — per-repository
content prepends, this document appends.
