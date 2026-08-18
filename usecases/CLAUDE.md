# usecases — application business rules

Interactors + Output Ports (traits). Interaction with outer layers goes only through the traits under `ports/`; implementations live in `adapters`, injection in `frameworks`.

## File map

| File | Contents |
|---|---|
| `supervisor.rs` | `Supervisor`: the landing point of all daemon orchestration; consumes `SupervisionRequest`/lifecycle events, yields `(result, Vec<SupervisionEffect>)` |
| `supervisor_ready.rs` | ready-probe orchestration (`on_ready`/`on_ready_timeout`/waiter cascade): the second impl block of `Supervisor`; shared fields and methods use `pub(crate)`; the second impl block needs `#[expect(clippy::multiple_inherent_impl)]` |
| `supervision.rs` | boundary contracts: `SupervisionRequest` / `SupervisionReply` / `SupervisionFailure` |
| `supervisor_log.rs` | `Supervisor`'s business logs (`log_*`); MUST be `pub` — `pub(crate)` inside a private module triggers clippy `redundant_pub_crate` |
| `timer_state.rs` | `TimerState`: the **business state** of timers/pending restarts/generation; holds no `JoinHandle` |
| `start.rs` | `start_apps` / `start_one`; `StartMode::{Register, Execute}`; `settle_start` (rollback verdict for the CLI-side start reply: refused→Partial, unsaved→Unsaved, otherwise Committed) |
| `stop.rs` / `restart.rs` / `delete.rs` | the CLI actions of the same name; `persist_for_handover` is daemon handover finalization (persist only, no state change) |
| `resurrect.rs` | after daemon restart, compares fingerprints service by service: adopt / evict / respawn |
| `supervise.rs` | child-exit and breaker supervision loop |
| `fingerprint.rs` | identity fingerprint assembly |
| `record.rs` / `persist.rs` | runtime records and persistence orchestration |
| `query.rs` / `table.rs` / `selector.rs` | queries, list data, `AppSelector` parsing |
| `log_paths.rs` | log path derivation |
| `handover.rs` | `pm3 install` before/after service comparison: `compare_handover` (adopted/restarted/lost) and `describe_handover`; pure functions. A survivor row **without** a pid is `lost` whenever it had one before — `resurrect` keeps the record (as `Stopped`, pid cleared), so "the name is missing" never happens and treating a pid-less row as uninteresting makes `pm3 install` exit 0 while services are dead |
| `ports/` | `clock` `dump_store` `fingerprint` `launcher` `log_rotate` `probe` `ready` `scheduler` `signaler` `wrapper` |

## Layer rules

- The "spawn at registration?" decision MUST live in the `StartMode::Register` branch of `start_apps`, **not** on the execution path — cron firing takes the execution path, and deciding there would make scheduled jobs never run
- **A `save_table` failure MUST NOT swallow an action already taken**: `restart_app` and `handle_child_exit` log the write failure as warn and still return their outcome (`persist_restart` / `log_unsaved_exit`). Returning `Err` there makes the caller skip `dispatch_restart` / `queue_restart` ⇒ no `WatchExit` for a process that is already running (autorestart and the memory breaker both dead, dump holds no pid ⇒ permanent orphan) or a decided `RestartAfter` silently dropped (every crashing service stops restarting on a full disk, with `pm3 list` reporting `stopped`). Because of this, `on_exit`'s guard makes `handle_child_exit`'s only `Err` unreachable there — consume it with `.expect()`, not a dead match arm
- Batch interactors MUST NOT use `?` inside the loop: `start_apps` returns `StartReport { outcomes, failure }` (not a `Result`); `resurrect` logs a warn per service and continues. A mid-loop `?` drops the already-spawned / already-adopted outcomes along with everything else, so the caller's `watch_all` and the final `save_table` never run → processes run with no watch task, autorestart and breaker both dead, the dump has no pid for them, and one daemon restart later they become permanent orphans
- When `resurrect`'s `topo_sort` fails it MUST degrade to "restore in table order" rather than aborting: dangling `depends_on` left by `delete` or missing service files both make the whole graph unsortable, and a single `?` abandons every surviving process. Same for `stop_all_apps` (MUST NOT fall back to `unwrap_or_default()`: stopping nothing while reporting success)
- `Supervisor::restart` runs `reload_declaration` first (`resolver.prepare` then overwrite `record.spec`) and then `restart_app`: an explicit `pm3 restart` must pick up hand-edited `<name>.yaml` and `<name>.env`. `on_restart` / `on_fire` / `restart_now` MUST NOT re-read from disk — cron and crash-triggered restarts must not fail because a file is temporarily unreadable. When the selector finds no record, `reload_declaration` returns `Ok(())` and lets `restart_app` report `NotFound` (keep the error source unique)
- The head of `resurrect` MUST `sweep_stranded` first (the origin of `stranded` and the token guard are in the root section "Environment variables and credentials"); `surviving_pid` / `evict_pid` are shared with the normal handover path — MUST NOT copy a bare `terminate`
- Force-kill has exactly one implementation, `Supervisor::sweep_pid` (both `on_force_kill` and `force_kill_survivors` go through it); `delete` MUST NOT `forget_generation`, or the generation guard breaks — see root `CLAUDE.md` "Processes and signals"
- `StartReport`'s `failure` (could not start) and `unsaved` (started but not persisted) MUST be carried as two separate fields; the protocol consequences and the contract "`Supervisor::start` returns `Err` only when `outcomes` is empty" are in the root "CLI ↔ daemon protocol" section
- When adding a Port trait, do not write a blanket impl: a file containing only trait declarations never enters lcov and trips the coverage gate's "production file missing" check → have the implementor write an explicit `impl Trait for X {}`
- `start_one` / `resurrect` involve the timing of identity-fingerprint capture and survivor eviction; the rules are in root `CLAUDE.md` "Identity fingerprints and reclaim" — read it before changing these two files
- `Supervisor` MUST NOT spawn/abort tasks itself: when a side effect is needed, push a `SupervisionEffect` and let `frameworks`' `TaskBoard` execute it. This layer does not know tokio; `TimerState` holds only business state and all `JoinHandle`s live in the outer layer — the two sides' fields correspond one to one, so adding an effect means changing both the `SupervisionEffect` enum and the match in `TaskBoard::apply`
- After adding a Port method you MUST implement it on the fake in `test_helpers/ports_test_helpers.rs` **and add a unit test**: the fake counts toward the coverage gate, so adding without testing creates uncovered lines

## Stop and force-kill

- `stop_all_apps` returns `Vec<StopOutcome>` (not a `Result`); persistence failure only logs a warn: a mid-way `?` would skip the loop that schedules `schedule_force_kill` for each outcome and the unswept sweep ⇒ services already SIGTERMed, memory already `Stopping`, yet no force-kill timer exists — `kill_timeout_ms` is dead letter
- In `stop_all`, a failed `terminate` MUST still `mark_stopping`: when the record stays `Online`, the unswept sweep still schedules a delayed force-kill for that pid, and the subsequent exit event goes through `classify_exit` → `decide_restart` and gets treated as a crash autorestart (symptom: minutes after `pm3 stop-all` the service comes back by itself)
- `on_force_kill`'s generation guard MUST yield **when a token is present**: a same-named `start` after `delete` bumps the generation, so the force-kill scheduled before the delete is judged stale and dropped ⇒ a stubborn old process runs alongside the new instance with no compensating path. With a token, `sweep_pid`'s `pid_was_recycled` guard covers it (more accurate than generation); only without a token does the generation guard stay (otherwise a bare signal could hit a recycled pid)
- `delete` MUST NOT clear the service's generation: `current_generation` returns sentinel `0` for unknown names and `is_current(name, 0)` is always true ⇒ the generation guard is dead letter; meanwhile a real exit event carrying generation≥1 arrives at `on_exit`, mismatches, and returns early, so `CancelForceKill` is never emitted ⇒ the delayed force-kill always runs the full `kill_timeout_ms` and may hit a recycled pid. Generation is a global monotonic counter — rebuilding a same-named service never collides, so it never needed clearing; in `on_exit` use "record no longer in the table" as the check and log debug
- `schedule_restart`'s `JoinHandle` is stored in `TimerBoard.restarts`; the three paths `stop`/`delete`/`stop_all` all call `cancel_restart`; `on_restart` does `claim_restart` before executing (events enqueued before the abort are thereby discarded): spawning a bare sleep task would let a stopped service resurrect itself and leave one orphan task per crash

## Ready probes

Semantics and terminal-state rules are in root `CLAUDE.md` "Ready probes"; here are this layer's three implementation constraints:

- **A record persisted as `Launching` that declares a probe MUST stay `Launching` and re-arm the probe** (`adopt` checks `status != Launching || ready_probe.is_none()` before `mark_online`): unconditional `mark_online` would show a service that underwent handover inside the probe window as online though it never became ready, and `await_ready_if_probing` would skip because the status is no longer `Launching` ⇒ the probe never runs and the timeout never fires
- Launching a waiter after its dependencies become ready (`launch_waiter`) **MUST `save_table` on the success path too**: persisting only on failure leaves a window of "process running but settled in the dump" — a daemon SIGKILL in that window means a permanent orphan plus a duplicate instance on the next start
- Multiple `depends_on` MUST all be registered: `DeferredStart.waiting_on` is a `Vec<String>` (`waiting_dependencies` collects all unready dependencies), and `launch_waiter` re-checks the remaining dependencies with `still_waiting` first. Registering only the first dependency would bring C (depends on A, B) online while B is still Launching, with no cascade when B's probe fails
- `stop`/`delete`/`stop_all` MUST go through `cancel_ready` (abort the probe task + remove from waiters + cascade-cancel), otherwise a dependency becoming ready will launch a Deferred service the user already stopped. **`stop_all` MUST call it for every name in the table, not for the `stop_all_apps` outcomes**: that list skips `is_settled()` records, and a Deferred waiter (and a dependency whose probe already failed) are exactly the settled ones — a later `start` of that dependency then revives a service the operator stopped
- **`launch_waiter` MUST launch through `register_one` (`StartMode::Register`), not `start_one`**: `Fire` mode bypasses the "a scheduled task is only armed at registration" branch, so `pm3 start dep job` executes a cron `job` the moment `dep`'s probe passes and again at its schedule. A `StartKind::Scheduled` waiter still counts as released (`releases_waiters`), or its own dependents wait forever
