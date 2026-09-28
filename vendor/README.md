# SurrealDB transaction compatibility patch

The MIT `LICENSE` files for both rquickjs crates are restored from the upstream
repository at commit `c99675e50201d74554d0f667094f800e1287001b`, matching their
`.cargo_vcs_info.json` metadata. Source:
https://github.com/DelSkayn/rquickjs/blob/c99675e50201d74554d0f667094f800e1287001b/LICENSE.
Cargo extraction markers (`.cargo-ok`) are excluded from version control.

`surrealdb-3.2.3` is the published SDK crate with its original licenses.
Archive SHA-256: `8d1d636f9a9104f4446943111e0703e6d6668bfaba6c4415a31d0450dc46630b`.
Its embedded explicit-commit handler erased all typed errors into `Internal`.
The patch preserves the concrete storage conflict and the core typed mapping,
so a confirmed conflict rollback returns 409 while uncertain failures remain 503.
Both Mem and SurrealKv regressions are in `crates/herta_db/tests/collections.rs`.
Remove the patch when the upstream SDK passes these tests without it.

`surrealdb-core-3.2.3` is the published crate, including its original licenses.
Archive SHA-256: `d6add99c7f91bda7acb5d3b9a1b7c0db81a1adebdaedd4b9145f9ccf65fa2a25`.

The SDK's `begin/query/commit` path does not install the notification broker that
the normal SQL executor installs. Consequently successful explicit transactions
do not notify existing LIVE SELECT subscriptions on either Mem or SurrealKv.

The local patch installs one broker on an externally owned transaction, retains
its notifications across queries, flushes only after a confirmed commit, and
discards them on cancellation or failed commit. Normal executor transactions
keep their original broker and behavior. Regression tests live in
`crates/herta_db/tests/transactions.rs`. Remove this patch after upgrading to an
upstream version that passes those tests without it.

## QuickJS continuation context patch

`rquickjs-sys-0.11.0` is the published crate, including its original licenses.
Archive SHA-256: `27344601ef27460e82d6a4e1ecb9e7e99f518122095f3c51296da8e9be2b9d83`.

Its bundled QuickJS emits Promise Before/After hooks for thenable assimilation,
but omits them for Promise reactions, including native `await` continuations.
That loses the caller's identity and transaction across asynchronous boundaries.
The patch captures a private Promise token when each reaction is registered and
emits Before/After around its handler. Tokens participate in reference counting
and GC marking and are never exposed to scripts. No tokens are allocated when
the host has not enabled Promise hooks. Continuation registration captures the
consumer's context, including when the awaited promise belongs to another branch.

The Windows runtime regression suite covers nested save with one worker, branch
isolation, Promise loops, and destruction. Keep the dependency at 0.11.0 until an
upstream implementation passes these tests without the patch.

The sys build script also tracks its `quickjs/` inputs so edits to the vendored C
sources invalidate Cargo's copied-source build output.

## rquickjs job exception ownership patch

`rquickjs-core-0.11.0` is the published crate with its original licenses.
Archive SHA-256: `b8bf7840285c321c3ab20e752a9afb95548c75cd7f4632a0627cea3507e310c1`.

`JS_ExecutePendingJob` returns a borrowed context on exception. The synchronous
and asynchronous Rust job-exception wrappers transferred that pointer into an
owning Context without incrementing its reference count. Dropping the exception
could therefore undercount the context and abort during garbage collection.
The three owning wrapper sites now duplicate the context first. The HTTP
afterCommit interruption regression reproduces the original assertion on Windows.
