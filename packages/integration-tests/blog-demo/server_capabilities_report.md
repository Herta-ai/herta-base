# Blog server capability report

This report is limited to behavior asserted by `tests/blog.spec.ts` against a real `hertabase` process and an in-memory database.

## Verified behavior

- A release server binary can be started on a dynamically selected loopback port, detected through the public OpenAPI endpoint, stopped, and started again without a fixed-port dependency.
- Bootstrap administrator login works, and the administrator can create base and auth collections.
- Auth registration and login return the standard response envelope. Auth user IDs and JWT `sub` claims use the full `collection:key` form.
- Schema-less collections retain extra fields while still validating declared fields.
- Relation writes use full IDs and reject bare keys, empty or malformed IDs, and IDs from the wrong target collection.
- Record paths accept either a full ID or a bare key. A full ID naming another collection returns 404.
- `$auth.record` supports native relation owner checks during list, view, create, update, and delete rules. `$auth.id` remains a full string ID and is used when rules compare request JSON.
- Public posts are visible anonymously. Private posts are visible to their author and administrators.
- Post authors cannot be forged during creation or transferred during ordinary updates. Cross-user updates and deletes return 403 `HB_FORBIDDEN`.
- `expand=author` keeps the original full relation ID and adds an authorized expanded record.
- Public-post comments are publicly visible. Private-post comments are limited to the post author, comment author, and administrators, and outsiders cannot create them.
- Administrators bypass collection business rules.
- Soft-deleted records disappear from GET and list results, and later PATCH or DELETE requests are rejected.
- Successes and failures use the documented `{ data, meta, error }` envelope, including stable HTTP status and Herta error codes.

## Not covered by this suite

- Persistent database engines, migrations from experimental string relation data, clustering, or multi-process behavior.
- Multi-tenant isolation, organization hierarchies, hierarchical administrator roles, or row-level policies beyond the blog rules above.
- Refresh-token rotation, account lockout, email verification, password recovery, OAuth, or realtime subscriptions.
- Multipart file storage, S3/LocalFS behavior, hooks, custom routes, deployment, backups, or performance/load limits.
- General PocketBase rule syntax such as `@request.auth.id`; this suite only verifies Herta's `$auth`, `$record`, and `$request` syntax.
- A soft-delete recovery API or public hard-delete API.
