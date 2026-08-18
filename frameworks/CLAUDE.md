# frameworks — entry points and assembly

`main.rs` + DI assembly + route binding + log initialization + lifecycle. No business logic, no format conversion.

**MUST NOT depend on `usecases`/`entities` (enforced by `arch_tests`): all inner-layer types come from the named re-exports in `adapters`.**

## File map

| File | Contents |
|---|---|
| `main.rs` | only calls `frameworks::cli`; no duplicate mod compilation |
| `cli.rs` / `commands.rs` | clap definitions and subcommand dispatch |
| `logs.rs` | `pm3 logs`: read-side aggregation (`cfg_dir` enumeration, stream selection, line prefixes, follow), does not go through the daemon; details in "pm3 logs" below |
| `daemon/` | `bootstrap` `actor` (event loop: hands events to `adapters::Supervisor`, then dispatches each returned `SupervisionEffect` to the `TaskBoard`) `timers` (`TaskBoard`: a pure `JoinHandle` table; spawn/abort cron timers, pending-restart tasks, force-kill delays, exit watchers, ready probes, memory sampling and log rotation ticks) `socket` (unix is `OwnerOnlyListener`, Windows is `PipeListener` named pipe + the `pm3.sock` existence-marker file, unified externally as the `Pm3Listener` alias) `service` `ports` |
| `client/uds.rs` | CLI-side socket client (`ask` / `ask_report`); transport forks per platform: unix is `UnixStream`, Windows is a named pipe (`connect_transport` returns `Box<dyn Transport>`; the HTTP codec is shared by both platforms) |
| `server.rs` | `serve_listener`: takes over an already-bound listener, avoiding the bind→drop→re-bind preemption window |
| `service.rs` | `pm3 startup` / `pm3 unstartup` |
| `install.rs` | `pm3 install`: backup, atomic binary swap, handover reinstall, takeover wait, before/after comparison (orchestration lives here; decision pure functions in `usecases::handover`, fs/manager probes in `adapters::install` and `adapters::unit`) |
| `signal.rs` | SIGINT swallowed, SIGTERM persists and exits; same API with dual implementation on Windows (CTRL_C swallowed, CTRL_SHUTDOWN persists and exits) |
| `layout.rs` / `telemetry.rs` / `prompt.rs` / `sandbox_probe.rs` | path layout, logging, interactive prompts, sandbox availability probing |

## Rules of this layer

### daemon orchestration

- No business decisions in this layer: `Daemon` only does "receive event → ask `Supervisor` → dispatch effects"; new behavior goes in `usecases/supervisor.rs`; this layer only adds one spawn/abort line in `TaskBoard::apply`
- The start lock MUST be held until the spawned daemon answers, not just until `spawn_daemon` returns (`ensure_daemon_running` releases it on every path afterwards): releasing early makes a concurrent CLI take the lock and spawn a second daemon during the bind window — harmless only because `bind_uds` reports `AlreadyRunning`, which is a backstop, not the mutual exclusion the lock is there to provide
- **"The socket is stale" MUST be proven, not assumed**: `bind_uds` unlinks only when the path is not a socket at all, or `connect` reports `ConnectionRefused`/`NotFound`. Treating every `connect` error as stale (EMFILE — the PM3-81 shape — EACCES, EINTR) unlinks a socket a live daemon still serves and lets a second daemon bind the new one: two supervisors writing one `dump.yaml`
- **`host_uid` MUST NOT rely on `/proc/self` alone**: macOS has no `/proc`, so it always reported `None` there and `launchctl_kickstart` (which needs `gui/<uid>/<label>`) silently did nothing — the documented launchd self-rescue was dead on the only platform with launchd. It now falls back to the owner of `$HOME`; a linux-gated test cannot catch this class, so assert it on a path both platforms have
- This layer MUST NOT signal processes itself: teardown sweeps go through `Supervisor::force_kill_survivors`, sharing `sweep_pid`'s identity guard with `on_force_kill` (rules in root "Processes and signals")
- One effect MUST have exactly one executor: `WatchExit`'s spawn also lives in `TaskBoard`; don't let `Daemon::run` intercept it early — that would make the corresponding `TaskBoard` match arm permanently unreachable and uncoverable (unreachable branches should be rewritten away, not masked with tests)

### CLI

- `main() -> Result<()>` prints errors with **Debug** (`Error: ServiceConflict {..}`) → MUST use `main() -> ExitCode` + explicit `eprintln!("{error}")`
- Global defaults MUST NOT be computed on the fly in `execute()`; hand them to clap `default_value_t` to compute at build time
  Reason: the e2e fake process is `pm3 __sleep`; after the child environment is wiped by `env_clear()` there is no `HOME`; "every subcommand resolves the config path first" would make the sleeper exit right at startup (symptom: in e2e the app shows `stopped ↺1`). Commands that don't read config are naturally unaffected
- CLI-side logging: `open_session` / `open_service_session` each call `init_cli_telemetry` once (writes to **stderr**; must not pollute stdout, which carries the reply). MUST NOT move it into `dispatch`/`execute` — `pm3 __sleep` doesn't read config (see previous item), `pm3 daemon` installs its own `LogSink::Stdout`; duplicate installation via `try_init` is absorbed by `.ok()`
- Early validation MUST use `pm3.search_path`, not `std::env::var("PATH")`; same for `sandbox_probe::detect_host_backend` (it MUST return the resolved `HostSandbox { backend, program }` absolute path, not just a bool: after the child's `env_clear()`, a bare `bwrap` is only looked up in `/bin:/usr/bin`; installed in `/usr/local/bin` every spawn reports ENOENT while the probe still claims the sandbox is available)

### pm3 logs

Service names come from enumerating `cfg_dir` filenames and taking stems (sorted, filtering out `.env` and non-yaml). Single-service output is verbatim with no prefix; aggregate mode prefixes each line with `<name> | `, and `--all`'s two streams use `<name> [out] | ` and `<name> [err] | `. Aggregate mode skips missing log files; single-service mode errors.

### Service files and rollback

- **Rolling back on a transport error MUST be limited to errors that prove the request never landed** (`request_never_landed`: connect failure, no daemon, and the daemon's own non-200 refusal): a `Stalled`/`Silent`/`Receive` failure means the daemon may have started and persisted the services, and deleting their service files makes the next handover's `resolve_service` fail ⇒ `stranded` ⇒ `sweep_stranded` evicts every running service. Those cases keep the files and log warn (`log_undecided_start`)
- When `start` is refused by the daemon, the already-written `cfg_dir/<name>.yaml` MUST be rolled back (`adapters::ServiceUndo` records the prior state: didn't exist → delete; existed → write back). But on partial daemon success, only the services in `ReplyDto.refused` MUST be rolled back (`undo.run_for`) — a running service must not lose its service file; see root `CLAUDE.md` "CLI ↔ daemon protocol"
- Writing to disk MUST NOT be moved after `ask` — the service file must already exist when the daemon persists `dump.yaml`
- Writing `~/.pm3/config.yaml` (the one `pm3 startup` writes) and writing `cfg_dir/<name>.yaml` share the same `adapters::reconcile`: identical content passes silently, different content prints a diff and refuses, `--force` overwrites. Any new "write config" path MUST go through it; don't start a second mechanism

### daemon teardown

- `clear_runtime_files` MUST delete `pm3.pid` before the socket: both `pm3 shutdown` and e2e judge teardown completion by "socket gone"; the reverse order lets "socket already gone, pid file still there" be observed (symptom: the `signal_semantics` e2e hangs on the pid-file assertion about 25% of the time)

### e2e (`tests/`)

Each e2e uses its own `PM3_HOME` tempdir. Existing coverage: full-lifecycle CLI chain, real sandbox isolation (writable inside cwd / denied outside cwd / network denied), crash circuit breaker, dependency start ordering and cycle detection, automatic persistence and `resurrect`, orphan socket self-healing, SIGINT swallowed and SIGTERM exit. When adding end-to-end behavior, first check here for an existing scenario to hang it on. Platform gating uses `#![cfg(unix)]` / `#![cfg(windows)]` on the file's first line (the latter currently only `service_windows.rs`: dry-run rendering + real schtasks register/unregister). **MUST NOT change the mount point to `#[cfg(all(test, unix))]`** (same for unit tests under `src/tests/`): clippy's `tests_outside_test_module` only recognizes `#[cfg(test)]` mods; changing the mount point makes every test function error

Techniques (shared by integration tests and e2e):

- Teardown MUST send SIGTERM and wait for exit: a SIGKILLed process never persists counters for already-executed lines (`LLVM_PROFILE_FILE` contains `%p`; each child writes its own profraw, but only on normal exit) → otherwise e2e covered lines are lost
- The teardown helper MUST unconditionally "first `pm3 list` to bring up the daemon and hand over, then `pm3 shutdown --with-services`"; writing "return if the socket doesn't exist" misses surviving children
- The fake process is the hidden subcommand `pm3 __sleep <ms>`, not `/bin/sh -c sleep`, to escape system shell differences; it is itself production code and MUST have one "spawn it, wait for normal exit, assert exit code 0" test
- A test target written as `sh -c` MUST include `exec` (`sh -c "exec sleep 30"`): without it sh only forks without exec, the signal hits sh and sleep becomes an orphan (symptom: nextest reports LEAK, the test stalls for the entire sleep duration)
- `sh -c "trap '' TERM; sleep 30"` does not reliably ignore SIGTERM when spawned by pm3 (manual shell and python spawn both work; the pm3 path doesn't, cause unknown) → don't use it as a "stubborn process" target; to cover the force-kill path call `on_force_kill` directly, or first use a fake `on_exit` to make the table believe the process already exited
- Asserting "dependency started first" cannot look at files the app itself writes (concurrent writes race); set `log_level` to debug and read the `"action":"spawn"` order from `pm3.log`
- Asserting "child environment was cleared" MUST probe `$HOME`, not `$PATH`: `/bin/sh` synthesizes a default value when PATH is missing
- Fake daemon (drives the CLI's decode-failure path): `UnixListener::bind(socket)` then reply `200` + non-JSON body (`tests/stale_socket.rs`); MUST use `while let Ok(..) = accept()` and MUST NOT `join()` — replying a fixed number of times hangs the test
- To test "calling the external service manager" (`launchctl`/`systemctl`/`loginctl`), use a `#!/bin/sh` script in a tempdir + `set_permissions(0o755)` as a stand-in, controlling both stdout and exit code; the only real binaries allowed are `/usr/bin/true`, `/usr/bin/false`, `/nonexistent/...`; **never** call the real `launchctl`/`systemctl` in tests
- Assertions on external command error text MUST be cross-platform: a valid but nonexistent pid reports `No such process` on both sides, while `illegal process id` exists only in macOS BSD kill; tests needing "a program that really exists" use `/bin/sh`, MUST NOT write `/opt/homebrew/...`
- `create_dir_all` in a fixture creates parent directories the test wants missing → store/source fixtures that construct error paths must take an independent root; don't derive it via `parent()` from the path under test
- `#[tokio::test(start_paused = true)]` (testing "timer fires event at deadline") needs `tokio = { workspace = true, features = ["test-util"] }` explicitly in dev-dependencies — the workspace's `"full"` does **not** include test-util, otherwise you get `no method named start_paused`; such tests MUST NOT use `timeout`-carrying helpers to wait for events (timeout is also auto-advanced and may fire first); call `events.recv().await` directly
- Testable pattern for interactive prompts (confirm): the loop signature takes `confirm: &mut (dyn FnMut(&str) -> bool + Send)`; production passes a fn that "locks stdin/stdout only per call" (`StdinLock` is not Send, MUST NOT be held across `.await`); tests pass scripted closures; MUST NOT touch real stdin in unit tests (under nextest stdin is null → immediate EOF, and answers can't be injected)
- Splitting a test file over 512 lines MUST mount the new file as a **child mod of the original test module** (`#[path = "x_tests.rs"] mod x;` at the end of the old test file, `use super::*;` at the top of the new one): a child module sees the helpers and fixtures `use`d by the parent test module; a sibling mod mounted on a production file does not

### Coverage (easiest to trip in this layer)

- File names under `src/tests/` and `src/test_helpers/` MUST end with `_tests.rs` or `_test_helpers.rs`: the gate's `listProductionSources` excludes test files by these two suffixes; any other name is treated as "production file missing from lcov" and fails
- Test helpers that can `panic!` MUST live in `src/tests/*_tests.rs` (llvm-cov ignores files whose path contains `tests/`); putting them in `test_helpers/` counts the panic branch as an uncovered line
- Functions "only driven via the real binary" MUST NOT get additional lib-side unit tests
  Reason: a lib test adds another instantiation; the function's remaining regions are permanently unreachable in that instantiation → gate fails (symptom: adding tests produces more missed regions; `--show-missing-lines` prints nothing while lines/branches are both 100%)
  Fix: delete the lib unit test; drive failure paths through e2e too (e.g. `pm3 --config /nonexistent shutdown`)
  Note: `main.rs` only calls `frameworks::cli` with no duplicate mod compilation, yet llvm-cov still counts two instantiations ("lib test binary + pm3 bin")
- New error branches go through the real binary: in e2e, `UnixListener::bind(socket)` a fake daemon that replies `200` + non-JSON body to drive the CLI's decode-failure path (`tests/stale_socket.rs`); such fake servers MUST use `while let Ok(..) = accept()` and MUST NOT `join()` (replying a fixed number of times hangs the test)
