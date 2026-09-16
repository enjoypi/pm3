# TODO

The single task list; delete entries when done. Project description in `docs/requirements.md`.

## Release

- [ ] `install.sh` 端到端验收：`HOME=$(mktemp -d)` 下跑两遍（首装分支用内置默认 config，升级分支沿用已有的），验下载、sha256 校验、解包与配置查找。仓库已公开，v1.16.0 产物已发布 ⇒ 前置条件已满足
- [ ] `windows-e2e` CI job 修复：夹具把 `SCRIPT` 写死 `/bin/sh`，Windows 上 `resolve_checked` 报 `ProgramNotFound`，fail-fast 让 1593 个测试全部没跑。改夹具按平台取程序路径（`/bin/sh` / `cmd.exe`），涉及 `adapters/src/test_helpers/apps_file_fixture_tests.rs`、`adapters/test_support/spec_sources_fixture_tests.rs`、`service_fixture_tests.rs`
