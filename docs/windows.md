# Windows Capability Matrix

The pm3 daemon and all CLI commands run on Windows 10 1803+. The auto-start form is a **current-user logon-triggered task in Task Scheduler** (no administrator required, semantically aligned with the user-level auto-start on macOS / Linux). Unix-only capabilities are degraded or rejected as listed below.

## Available

| Capability | Notes |
|---|---|
| `pm3 startup` / `pm3 unstartup` (including `--dry-run`/`--force`) | Auto-start configuration lives under `~/.pm3/service/`; starts immediately after registration |
| `pm3 startup --status` | Query the run state of the registered task |
| `pm3 install` (backup and generational replacement) | Same chain as on Unix; takeover is judged by the pid file (Task Scheduler has no notion of a manager pid) |
| daemon / CLI communication | Named pipe; the pipe name is mixed with a random owner-readable-only secret under `~/.pm3`, so other users can neither predict it nor preempt its registration |
| `pm3 start/stop/restart/delete/list/logs/shutdown` | Stop and force-kill take the whole process tree along, equivalent to Unix process-group semantics |
| Crash self-healing | The task is restarted on failure, equivalent to "always restart" |

## Degraded (behavior differs from Unix)

| Capability | Difference |
|---|---|
| `pm3.service.restart_condition` | `on-failure` is degraded to `always`: exit semantics cannot be conveyed to Task Scheduler |
| `pm3.service.restart_delay_secs` | Task Scheduler's minimum restart interval is 1 minute; smaller values are raised to 60 seconds |
| `pm3.service.max_tasks` / `cpu_quota_percent` / `wait_for_network` | Task Scheduler has no counterparts; ignored at render time |
| TERM / KILL | Windows has no signal semantics, so force-kill treats both alike; graceful shutdown relies on the daemon's own console shutdown event (persist, then exit) |
| File permissions | No 0600/0700 chmod (NTFS user directories are per-user by nature); `.env` permission tightening and socket owner checks are both skipped |
| Process identity token / liveness probing | Without `/bin/ps` everything takes the "unreadable" path: services are always restarted after a daemon replacement, memory circuit-breaking does not work, and the CPU/RSS columns of `pm3 list` are empty |
| Log rotation detection | No inodes, so rename-style rotation is not detected; copytruncate (truncation) is still recognized |

## Not supported (fail-fast)

| Capability | Behavior |
|---|---|
| Sandbox (`sandbox.mode` set to `read-only` / `workspace-write`) | No seatbelt/bwrap counterpart; startup fails with `no sandbox backend`. On Windows, `pm3.sandbox.mode` must be set to `danger-full-access` |
| Peer credential check | Named pipes have no `SO_PEERCRED` equivalent; the unpredictable pipe name covers it instead (see the table above) |
| Auto-start status query under a non-English locale | Status parsing assumes English output; on a non-English system `pm3 startup --status` always reports `installed, not running` |

## Windows-specific configuration

- `pm3.service.schtasks_path` (default `schtasks`, resolved via PATH) — path of the auto-start manager, following the same convention as the manager paths on other platforms
- `pm3.service.taskkill_path` (default `taskkill`, resolved via PATH) — path of the process tool used for stop and force-kill; on unix this is the hardwired `/bin/kill`, hence no corresponding key
