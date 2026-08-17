# Blog integration test

This package starts a release `hertabase` binary against an in-memory database and runs the process-level blog contract with Vitest. The test creates its collections and users from scratch, so repeated runs do not preserve data.

Prerequisites: Node.js 20 or newer and pnpm. From the repository root:

```sh
pnpm test:integration
```

The package `pretest` builds `target/release/hertabase` before Vitest starts it. The test server chooses a free loopback port, polls the public OpenAPI endpoint for readiness, and terminates the complete process tree after the suite.
