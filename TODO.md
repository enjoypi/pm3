# TODO

The single task list; delete entries when done. Project description in `docs/requirements.md`.

## Release

- [ ] `install.sh` 端到端验收：`HOME=$(mktemp -d)` 下跑两遍（首装分支用内置默认 config，升级分支沿用已有的），验下载、sha256 校验、解包与配置查找。仓库已公开，v1.16.0 产物已发布 ⇒ 前置条件已满足

## 覆盖率（按平台）

- [ ] macOS/Linux：跑 `/rust-cov-100` 确认仍为 100%（本轮只在 Windows 上验证过，unix 侧只过了 `--target x86_64-unknown-linux-gnu` 的 clippy）
- [ ] Windows：从基线 functions 1459/1683、branches 918/1090（2026-09-27 UTC）推到 100%。缺口来自 `frameworks/tests/` 24 个 `cfg(unix)` e2e 未移植，按缺口大小依次移植：`logs_streams`（`logs.rs` 缺 23/32）→ `daemon_lifecycle`（`commands.rs`、`daemon/bootstrap.rs`、`daemon/socket.rs`、`cli.rs`）→ `install`（`install.rs`、`adapters/src/install/store.rs`）→ 其余。adapters 侧 `apps_file/env_file.rs`、`process/watcher.rs`、`unit/runner.rs` 的缺口来自单测里的 `cfg(unix)` 块
