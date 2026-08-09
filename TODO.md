# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

- [ ] 在 macOS 上跑一次 `just lint` + `just cov --fresh` 复核：四处平台性缺口的修法（`layout.rs` 的 `owner_uid_of`、`ps_probe.rs` 的重探测试、`watcher.rs` 的自计数 fake ps、`private_file.rs` 的 `fill`）都是在 Linux 上验证的，两平台应同为 100%

## 发布与用户安装方案

现状：README 已写三条安装路径——`install.sh` 一行装（curl|sh，sha256 校验后自动 `pm3 install`）、GitHub Releases 手动下载（产物带 `config.yaml` 与 `.sha256`）、`cargo install --git <url> --bin pm3 --locked`（已实测 virtual workspace 下 `--bin` 能定位）。**前提：仓库转 public**，私有状态下匿名下载全部 404。crates.io / Homebrew / cargo-binstall 已拍板不发布。

- [ ] install.sh 真机验收：v1.11.2 已推送触发 release.yml（含 .sha256、config.yaml、Windows 产物与 windows e2e job；v1.11.1 缺 zip 应删除）；确认 release 产出后，`HOME=$(mktemp -d)` 跑 install.sh 验首装路径，再跑一次验升级路径（匿名下载仍需仓库转 public）

## Windows 收尾

PM3-59 已合入并发版（v1.11.0）：`pm3 service install/uninstall/status` 与 `pm3 install` 换代链在 Windows 走 Task Scheduler + 命名管道，能力矩阵见 `docs/windows.md`。剩余验收项：

- [ ] Windows 真机验收：`service install --dry-run` 目检 → 注册/`/Query` → start/list/logs/stop → 注销重登自启 → `pm3 install` 换代 → uninstall 清场（`frameworks/tests/service_windows.rs` 已备好同路径 e2e；release.yml 已有 windows-latest job 在 tag 时自动跑这份 e2e）
  - 重点疑点：`pm3 install` 默认落位 `~\bin\pm3` 无 `.exe` 后缀（`adapters/src/install/layout.rs` 的 `DEFAULT_DESTINATION`），Windows 上无扩展名不可执行，验收时确认并修
- [ ] schtasks 非英文 locale 下 `/Query` 输出解析失效（状态恒报 not running），需要时换 PowerShell `Get-ScheduledTask` 的对象化输出
