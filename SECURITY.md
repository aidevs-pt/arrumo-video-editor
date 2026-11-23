# Security policy

## Supported versions

Security fixes go into the latest release of Arrumo AI Video Editor. Arrumo updates itself, so please
make sure you can reproduce the problem on the latest version.

## Report a vulnerability

**Please don't report security problems in public issues, discussions or pull requests.**

Report privately, either:

- through GitHub: **Security › [Report a vulnerability](https://github.com/aidevs-pt/arrumo-video-editor/security/advisories/new)**, or
- by email to [security@aidevs.pt](mailto:security@aidevs.pt).

Include what you found, the affected version and platform, steps to reproduce it, and the impact
you expect. Never include real API keys, tokens or someone else's data; redact them.

## What happens next

- We acknowledge your report within **3 working days**.
- We confirm the problem and agree on a fix and a disclosure date with you, usually within
  **90 days**.
- We publish a security advisory with the fixed release and credit you, unless you prefer to stay
  anonymous.

## Scope

In scope: this repository's code and its releases, including how Arrumo stores credentials, runs
plugins in isolation, checks plugin packages and handles files and links from agents and plugins.

Out of scope: vulnerabilities in third-party services that plugins connect to (report those to the
provider), and problems that need an already compromised computer or account.

Thank you for helping keep Arrumo and its users safe.
