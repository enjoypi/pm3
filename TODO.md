# TODO

The single task list; delete entries when done. Project description in `docs/requirements.md`.

## Release

- [ ] `install.sh` real-machine acceptance: run once with `HOME=$(mktemp -d)` to verify the first-install path, then once more to verify the upgrade path. **Prerequisite: the repo goes public** — while private, anonymous downloads all 404

## Windows wrap-up

- [ ] Make the full `windows-e2e` suite green: that job in `release.yml` runs the whole workspace nextest; the failure surface is fixtures hardcoding unix paths like `/bin/sh` and `/tmp`. fail-fast hides the full failure surface → first get the complete list with `--no-fail-fast`, then platform-ize fixtures in batches (no local Windows, CI iteration only). Once green, add `windows-e2e` back to `release.needs` (temporarily removed for releases)
- [ ] Windows real-machine acceptance: `startup --dry-run` visual check → register / `/Query` → start/list/logs/stop → log off and back on to verify autostart → `pm3 install` handover → `unstartup` cleanup (`frameworks/tests/service_windows.rs` already has the same-path e2e ready; `release.yml` runs it automatically on tags)
  - Prime suspect: `pm3 install`'s default destination `~\bin\pm3` has no `.exe` suffix (`DEFAULT_DESTINATION` in `adapters/src/install/layout.rs`); extensionless files are not executable on Windows — confirm and fix during acceptance
- [ ] `schtasks /Query` output parsing breaks under non-English locales (status always reported as not running); switch to PowerShell `Get-ScheduledTask`'s object output when needed

