# entities — enterprise business rules

No I/O, no async, no framework dependencies (except the `serde`/`thiserror` derives).

## File map

| File | Contents |
|---|---|
| `process/spec.rs` | `AppSpec` and `validate_spec` |
| `process/status.rs` | `ProcessStatus` state machine (`is_running` / `is_settled`) |
| `process/restart.rs` | restart policy and breaker counting |
| `process/runtime.rs` | runtime fields (pid, identity token, restart count) |
| `process/depgraph.rs` | dependency startup ordering and cycle detection |
| `sandbox/policy.rs` | `SandboxPolicy`: declared `writable_roots` / `readable_roots`, pm3-derived `derived_roots` / `derived_readable_roots`, unions via `granted_roots()` / `readable_grants()` |

## Layer rules

- The breaker condition is `unstable_restarts >= max_restarts` (aligned with pm2 `God.js`); MUST NOT change it back to `>`
- When a new variant is added to `ProcessStatus`, re-audit both `is_running()` and `is_settled()`: they are not complementary — `Stopping` satisfies neither
- Every declared root field MUST come in a pair: the declared one (fingerprinted) plus a `derived_*` twin (not fingerprinted) holding the canonical path, and a backend MUST read only the union accessor — a backend reading the declared field alone silently loses the grant a symlinked path needs
- The `writable_roots` / `derived_roots` split in `SandboxPolicy` is the foundation of "daemon handover must not misjudge respawn"; before changing it, read the root `CLAUDE.md` section "Identity fingerprints and reclaim"; the blast radius of adding a field is in the root "Change ripple checklist"
