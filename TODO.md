# TODO

The single task list; delete entries when done. Project description in `docs/requirements.md`.

## Release

- [ ] `install.sh` 端到端验收：`HOME=$(mktemp -d)` 下跑两遍（首装分支用内置默认 config，升级分支沿用已有的），验下载、sha256 校验、解包与配置查找。仓库已公开，v1.16.0 产物已发布 ⇒ 前置条件已满足

## 覆盖率（按平台）

- [ ] Linux：跑 `just cov` 确认仍为 100%（macOS 已于 1.22.0 确认；Linux 未验证）。同时确认两处只在 macOS 上写出来的行为：`sandbox.listen` 启动时报 `ListenUnsupported`；`sandbox_isolation` 断网用例的 `nc -v` 输出含 `Network is unreachable`
- [ ] Windows：从基线 functions 1459/1683、branches 918/1090（2026-09-27 UTC）推到 100%。缺口来自 `frameworks/tests/` 24 个 `cfg(unix)` e2e 未移植，按缺口大小依次移植：`logs_streams`（`logs.rs` 缺 23/32）→ `daemon_lifecycle`（`commands.rs`、`daemon/bootstrap.rs`、`daemon/socket.rs`、`cli.rs`）→ `install`（`install.rs`、`adapters/src/install/store.rs`）→ 其余。adapters 侧 `apps_file/env_file.rs`、`process/watcher.rs`、`unit/runner.rs` 的缺口来自单测里的 `cfg(unix)` 块

## 测试与 lint

- [ ] 其余新写可执行脚本的测试（`rg -l "0o755" adapters frameworks`，约 20 个文件）改用仓内固定脚本或 `sourcing-launcher` 符号链接，原因见根 `CLAUDE.md` 构建环境一节
- [ ] `daemon::actor` 的 `a_running_service_reports_its_resource_usage` 在一轮 `just cov` 中偶发失败过一次，未复现、原因未查明
