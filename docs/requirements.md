# pm3 Requirements

pm3 is a minimalist pm2 with strict sandbox isolation on top. A single program serves as both the command-line tool and the resident daemon; the two communicate directly on the local machine without occupying any network port.

It targets the scenario of "hosting a few long-running programs and scheduled jobs on your own machine or a single server": managed programs auto-start at boot, are restarted automatically after crashes, have their logs collected in one place, and can only read and write their own little patch of ground.

## Why pm3

pm2 solves "keep a program running and restart it automatically," but it does not care what those programs can touch: a managed process can by default read the whole disk and reach the whole network, so one compromised little service is an entry point into the entire machine. It also carries a full language runtime and a hundred megabytes of resident memory — raising one big process just to manage three small ones.

pm3 wants the opposite defaults: **manage bare processes, deny the network by default, allow writes only to the working directory by default**; every opening must be declared one by one. The manager itself is just a resident process of a few MiB and depends on nothing beyond the operating system's own process-query and signal tools.

It is not trying to replace containers. Containers give you image distribution and strong isolation — dependency packaging, an independent network stack, reproducible deployment; pm3 gives you lightweight hosting of bare processes — second-level startup, direct reads and writes of host files, no image-building step. The pm3 sandbox is "dropping privileges," not virtualization: services share the kernel with the host, isolation is weaker than a container's, but the overhead is negligible. If you need to distribute to other people's machines, reproducible environments, or strong isolation, use containers; if you want to run a few long-lived small services and scheduled jobs on your own machine, use pm3.

Likewise, for managing large clusters or in-process load balancing, pm2 is the better fit. pm3 deliberately does not do those things.

## Managing a program

```
pm3 start --name web [options] <program> <arguments...>
```

The command does two things: it records this launch intent as a configuration file dedicated to that service, then asks the daemon to run the program according to it. From then on `pm3 restart web` / `stop` / `delete` / `describe` recognize only the service name; to read logs use `pm3 logs web`, which prints the last few lines and then keeps following new output; `-n` changes the number of lines, `--nostream` prints the tail and exits without following.

The configuration file is meant for humans to read and is allowed to be edited by hand: it contains no absolute paths — the home directory is written as `${HOME}`, and the program is written as a bare name rather than a full path. That way the same configuration still holds on another machine or under another username.

`pm3 list` shows every service's status, restart count, and next scheduled trigger time on one screen.

`pm3 stop|restart|delete|reset all` applies the operation to every managed app at once; `all` is a reserved name and cannot be used as an app name.

`list` has the aliases `l`/`ls`/`ps`/`status`, and `describe` has the aliases `desc`/`info`/`show`.

## Environment variables and credentials

A service's environment variables all live in a `<service-name>.env` file next to the service configuration, one `KEY=VALUE` per line, with `#` starting a comment. If the file exists it is loaded automatically; if it does not exist the service simply has no environment variables. The configuration file itself **does not accept** an `env` field — writing one is an outright error with a hint to move it to `.env` — so credentials never appear in the configuration file, in `pm3 describe` output, in runtime state records, or in any log line.

Besides what this file declares, pm3 also hands `HOME` to the service so the configuration need not hardcode absolute paths; declaring your own `HOME` in `.env` takes precedence. Beyond that, the environment the service receives is empty.

Values may contain `$HOME` or `${HOME}`, which pm3 replaces with the home directory above, so `PATH=$HOME/.cargo/bin:/usr/bin:/bin` works without hardcoding a username. Every other `$` is passed to the program exactly as written — `$HOMEBREW_PREFIX` is not mistaken for the home directory, and a `$` inside a random password is never rewritten; if a password really does contain the characters `$HOME`, wrap the value in single quotes and it is copied verbatim.

Every time pm3 reads this file it tightens its permissions to owner-read-write only, and the configuration directory itself is open to the owner only; if the file is a symlink it is only read, without touching its permissions, so as not to disturb someone else's credentials.

Rotating credentials means editing the file and running `pm3 restart <service-name>` — a restart re-reads both the configuration and `.env`. The daemon also re-reads them when it is replaced by a new generation, and if the credentials changed the service is restarted with the new values. Deleting a service deletes its `.env` along with it.

If the file is broken (say, a missing `=`) when the daemon happens to be replaced, pm3 can no longer tell how that service is supposed to run, so it stops the original process and explains why in the log, rather than leaving it alone — otherwise it would keep running in the background beyond anyone's control, and the next start would spawn a duplicate.

## Intent and runtime state are stored separately

The **intent** the user declares (what to run, with which arguments, which directories it may write) and the **runtime state** pm3 records (current pid, restart count) are stored separately and never pollute each other. Deleting runtime state does not lose configuration; hand-editing a configuration file does not corrupt the records of running processes.

## Sandbox isolation

By default each managed program can only write to its own working directory — writes outside it are rejected by the system, and network access is rejected too. Directories that need to be opened are declared one by one by the operator in the service configuration. Isolation is enforced by the operating system kernel, not blocked by pm3 in user space.

Reads are restricted as well: by default a service can only read system directories (the whitelist in the configuration), its own program files, and its own working directory; other paths are invisible. If a program genuinely needs to read elsewhere, declare readable directories one by one (`--readable-dir`); a service that really cannot be contained can have reads opened up to the whole disk in its configuration.

No matter how far reads are opened, pm3's own two directories — the one holding runtime state and the one holding service configurations — are always carved out of every sandbox: one service cannot read another service's credentials, nor see the daemon's communication channel. Consequently a service's working directory must not be set to one of pm3's own directories; such a declaration is rejected outright.

Programs that bring their own sandbox (sshd, for example) cannot fit inside pm3's sandbox — the system does not allow sandbox nesting, and such a program is rejected when it initializes its own layer. These programs must turn the sandbox off entirely in their configuration and rely on their own isolation mechanism.

To refer in arguments to "this service's own writable working directory," write the `${PM3_SERVICE_CWD}` placeholder, which pm3 replaces with the real path at startup.

## Crashes and circuit breaking

A program that exits abnormally is restarted automatically. If it crashes repeatedly within a short time and reaches the threshold, retries stop and the service is marked errored, avoiding a pointless restart storm. Services may declare dependencies on each other; pm3 starts them in dependency order, and reports an outright error on a cyclic configuration instead of looping forever.

The retry interval for repeated crashes grows with exponential backoff: the first unstable restart uses the restart interval from the configuration, then each subsequent one doubles, capped by default at 15 seconds; when the interval is configured as 0 there is no backoff, and behavior is identical to a fixed interval.

Dependency order only guarantees start order, not that the depended-upon service is actually serving. When the latter is needed, give the service a ready probe, in either of two forms: periodically execute a command, and exit code 0 means ready; or periodically connect to an address and port, and a successful connection means ready. An overall readiness budget may also be given, defaulting to 30 seconds. The probe runs on the host, outside the sandbox — it probes availability from the client's point of view, so a service with no network inside its sandbox can still be probed.

A service with a probe stays "launching" after start and only turns "online" once the probe passes; a service that declares dependencies is first recorded as "queued" and is only really launched once the one it depends on is ready. On timeout, or when the probe command itself does not exist, the service is stopped and marked errored, and services queued waiting for it are cancelled along with it — probe failure is a terminal state and does not trigger auto-restart. If the daemon is replaced while a probe wait is in flight, queued services are persisted as stopped and are not automatically resumed after the handover; a manual `start` is required.

Memory leaks also have a backstop: give a service a memory limit (`--max-memory-restart 300M`), and pm3 periodically samples its resident memory and restarts it when it exceeds the limit. Services without a configured limit are unaffected and are not sampled.

## Scheduled jobs

Writing a 5-field cron expression in the service configuration schedules it:

- With "no auto-restart," it is a one-shot job: it runs once at the scheduled time and finishes
- With "auto-restart," it is a resident service that gets restarted once at the scheduled time

OpenBSD-style random syntax `~` is supported: `~` picks a random value for that field, `a~b` picks randomly within the range, and `a~b/n` picks randomly within the range with a step. A new value is drawn after every trigger, so a requirement like "twice a day, morning and evening, at a random minute" lands on different minutes each time — a way to spread the load of scheduled jobs.

The `next` column of `pm3 list` shows the next trigger's local time; when this column is empty the service really is stopped — there is no pm2-style ambiguity of "the service is stopped but the timer is still running."

## Who may issue commands

The command channel is an entry point in the filesystem, readable and writable by the owner only, in a directory open to the owner only. On top of that, the daemon checks which user is actually on the other end of each incoming connection; if it is not itself, it disconnects directly and logs a line — giving the other side no chance to speak. Requests also have a size limit, so one malformed request cannot jam command processing.

## Taking over existing services after a daemon restart

Upgrading pm3 itself or restarting the daemon should not interrupt services that are already running. After restarting, the daemon checks them one by one: if a service's process is still the original one, and neither its launch arguments nor its program file have changed, it is adopted and monitoring continues; if any item does not match, the old process is stopped first and the service is restarted with the new configuration.

A host reboot is a different matter: those process ids have long since been handed out to other programs by the system, and verifying identity is meaningless. pm3 records "this boot" in its runtime state, and when it finds the recorded boot is not the same one, it treats every process id in the records as invalid, simply starts everything again from configuration, and sends not a single signal outward. When upgrading from an old version (where the state has no such record yet) or when the boot information cannot be read on this machine, it falls back to the old rule of verifying identity one by one, and never restarts all services for no reason.

Sending the daemon a termination signal only saves state and exits; it does not touch the managed services. To stop the services along with it, use `pm3 shutdown --with-services`.

## Auto-start at boot

```
pm3 startup               # register as an auto-start service for the current user
pm3 unstartup             # unregister, without deleting configuration
pm3 startup --status      # show the current registration status
pm3 startup --dry-run     # only print what would be written
```

On all three platforms the registration is an auto-start entry for the **current user**, requiring no administrator privileges. On Linux, "keep running while the user is not logged in" is also enabled. Some capabilities on Windows differ from the other two platforms; see the Windows notes.

The auto-start configuration also carries a process-count limit: when a managed program runs away replicating itself, it hits this wall instead of exhausting the whole machine's process table. The limit is the combined total of pm3 plus all its services, and is tunable in the configuration. To also constrain CPU, configure a percentage quota; this item is supported on Linux only — what macOS can limit is cumulative CPU time, which on a resident service amounts to killing it when the time is up, so it is not offered.

Upgrading pm3 itself is a single command, `pm3 install`: it first backs up the current generation (the binary, the auto-start configuration, and pm3's own configuration, archived under the old version number), swaps in the new binary, reinstalls the auto-start entry, then verifies one by one that the new generation has really taken over every service — a single service not taken over means failure. Rolling back means fetching that trio back from the backup directory.

## Where files live

| Location | Contents |
|---|---|
| `~/.pm3` | Runtime state: the communication entry point, process id records, logs, each service's working directory, the daemon's own configuration, and the rollback backup from each upgrade |
| `~/.config/pm3` | One configuration file per service, plus an optional credentials file of the same name |

Both locations can be changed in the configuration. When writing a configuration file whose target already exists: identical content passes silently; different content prints a diff and refuses to overwrite; only with `--force` is it really written.

`pm3 config check` validates that the daemon's own configuration parses, and `pm3 config show` prints the final result after placeholder substitution — it is the first thing to look at when troubleshooting "what the configuration says differs from what actually takes effect."

The program search path is decided uniformly by pm3's own configuration and is not inherited from the environment variables of the shell that launched pm3, avoiding "it runs when started by hand, but auto-start at boot cannot find the program."
