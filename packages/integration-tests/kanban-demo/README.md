# Kanban integration test

This package starts an isolated HertaBase process with an in-memory database and verifies a collaborative Kanban contract: workspace membership rules, private tasks, native relation filters and deep expansion, filtered SSE updates, comments, and attachment append/replace behavior.

From the repository root, run the debug build used during development:

```sh
pnpm --filter kanban-demo-integration test
```

After Rust changes have passed in debug, run the slower release verification once:

```sh
pnpm --filter kanban-demo-integration test:release
```

`HB_TEST_PROFILE=debug|release` can select an already-built profile explicitly. The runner never falls back to a different profile.
