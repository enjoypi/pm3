@~/.claude/CLAUDE.md

@~/.claude/clean-architecture/CLAUDE.md

@~/.claude/rust/CLAUDE.md

pm3: a minimal pm2 with strict sandbox isolation. One binary is both the CLI and the long-running daemon, talking over a unix socket (a named pipe on Windows). CLAUDE.md records only pitfalls; cross-layer ones live here, single-layer ones sink into each crate's own CLAUDE.md.

## Layout and runtime files

- **macOS and Linux keep the identical XDG layout on purpose** (one layout = one test suite = one migration doc); the platform difference is absorbed by probing whether `/run/user/<uid>` exists, never by branching on the operating system. MUST NOT switch macOS to native `~/Library/...`
- **MUST NOT derive any pm3 path from macOS's `$TMPDIR`**: a launchd-started daemon has it and a shell-started CLI does not ⇒ the two compute different socket paths ⇒ the CLI cannot connect, `ensure_daemon_running` spawns a second unmanaged daemon, and two supervisors write one `dump.yaml`
- **The socket and the pid file MUST carry the sticky bit** (`adapters::write_sweep_proof` / `SWEEP_PROOF_FILE`, 0o1600): the XDG spec lets `XDG_RUNTIME_DIR` be swept by atime, and nothing in pm3's periodic ticks touches the socket ⇒ an idle daemon's socket disappears while the daemon still holds the unlinked inode ⇒ the next CLI command gets NotFound and spawns a competitor. Sticky beats a "touch it every few hours" tick: no new timer, no new coverage region. Its side effect is exactly the wanted one — neither file survives a reboot, which is what `PidTrust` assumes
- `$XDG_*` is read **once**, at `pm3 startup`, and frozen into the unit as absolute `PM3_{CONFIG,STATE,RUNTIME,DATA}_DIR`: `host_pm3_env()` only filters the `PM3_` prefix, so a launchd/systemd daemon never sees `XDG_STATE_HOME` while the operator's shell does. pm3 internally honours `PM3_*` only

## Diagnostics

- `unstable_restarts` (what the breaker judges) is recomputed only when a process exits, so a service that has been healthy for hours keeps a stale tally. The liveness tick clears it through `usecases::query::settle_stability` once the record has been up past its `min_uptime_ms`. That function MUST stay a **non-generic** function in `query.rs`: written inside `Supervisor::on_*` its regions split across the `impl Ports` instantiations and no amount of tests fills them
- `pm_id` starts at **1** and reuses the gaps deleted services leave (`ProcessTable::lowest_free_pm_id`). `ProcessTable` therefore keeps no `next_pm_id` cursor — the records are the only source of truth, so a sparse `dump.yaml` cannot push new ids to the far end
- liveness watching MUST skip `autorestart: false` services: the failure path is stop → breaker → `GiveUp` → `Errored`, so watching a service the operator asked pm3 not to heal only kills it
