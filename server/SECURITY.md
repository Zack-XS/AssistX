# Security Policy

This project runs a personal assistant that can read mail, manage calendars, spend money and remember private details. We take security reports seriously.

The design and the full list of threats we defend against are in [docs/06-threat-model.md](docs/06-threat-model.md).

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Report privately by one of these:

1. GitHub: use **Security → Report a vulnerability** on this repository (private vulnerability reporting).
2. Email: `security@<PROJECT-DOMAIN>` <!-- TODO: set a real address -->

Please include:

- What you found and what an attacker could do with it.
- Steps to reproduce, or a proof of concept.
- The version or commit you tested.
- Your setup: OS, model adapter, plugins involved.
- Whether you want credit, and under what name.

Do not put real secrets or other people's private data in a report. Use test accounts.

Flaws in the design docs count too. You do not need working code to report one.

## What to expect

| Step | Time |
|---|---|
| We confirm we got your report | within 7 days |
| We tell you if we accept it and how serious it is | within 14 days |
| Fix released for high and critical issues | target 30 days |
| Fix released for low and medium issues | next planned release |

We will keep you updated as we go. Once a fix is released, we publish an advisory, request a CVE where it fits, and credit you unless you ask us not to.

If a report shows active attacks against users, we may release a fix and advisory sooner.

## Scope

**In scope**

- The core engine and API (this repository).
- The sandbox, network proxy and Safety Gate.
- Built-in plugins: `plugin-maker`, `memory-tools`, `scheduler`.
- Built-in prompts in `prompts/`.
- Release builds and install scripts we publish.

Examples of issues we want to hear about:

- Getting a risky action (send, spend, delete, share) to run without an approval.
- Prompt injection that bypasses the Safety Gate, not just injection that fools the model's words.
- Escaping the plugin sandbox, or reaching a host not listed in a plugin's `needs`.
- Calling the API without a valid token, or doing owner actions with a `client` or `trigger` token.
- Bypassing the `Host` or `Origin` checks (DNS rebinding, CSRF).
- Reading secret values from the model, logs, tool output or exports.
- Memory poisoning that gets outside content into `core.md` or stored as a user preference.
- "Forget" leaving copies of the forgotten data, including in the action log or stored events.
- Approval cards that show something different from what actually runs.
- Memory trust labels that the model can raise, or a plugin that edits its own code, status or tags after approval.
- A plugin whose tags or `needs` hide what it really does and get past the approval card.

**Out of scope**

- Third-party plugins not shipped in this repository. Report those to their authors.
- Front ends built by others on top of the API.
- The model saying something wrong or odd, when no action runs without approval.
- Attacks that need root or full control of the user's device.
- A malicious model provider.
- Denial of service by the user against their own install.
- Missing hardening with no clear way to attack it.

## Supported versions

Before 1.0, only the latest release gets security fixes.

| Version | Supported |
|---|---|
| latest `0.x` | yes |
| older | no |

After 1.0, the latest minor release of each supported major version gets fixes.

## Safe harbor

We will not take legal action against you for security research that:

- Tests only installs and accounts you own or have permission to test.
- Does not access, change or delete other people's data.
- Does not harm others or degrade services for them.
- Gives us a fair time to fix the issue before you share it publicly. We suggest 90 days, or sooner once a fix is released.

## For users: staying safe

- Keep the API bound to `127.0.0.1` unless you need otherwise. If you expose it, use TLS.
- Use a `client` token in front ends. Keep the owner token for owner actions, and do not paste it into other apps.
- Keep tokens out of URLs and shell history.
- Read the approval card before installing a plugin, especially its network hosts and secrets.
- Install plugins only from sources you trust.
- Keep your install updated.
