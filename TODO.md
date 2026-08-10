# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## 发布与用户安装方案

现状：README 已写三条安装路径——`install.sh` 一行装（curl|sh，sha256 校验后自动 `pm3 install`）、GitHub Releases 手动下载（产物带 `config.yaml` 与 `.sha256`）、`cargo install --git <url> --bin pm3 --locked`（已实测 virtual workspace 下 `--bin` 能定位）。**前提：仓库转 public**，私有状态下匿名下载全部 404。crates.io / Homebrew / cargo-binstall 已拍板不发布。

- [ ] install.sh 真机验收：v1.11.3 已推送触发 release.yml（含 .sha256、config.yaml、Windows 产物与 windows e2e job；v1.11.1 缺 zip 应删除）；确认 release 产出后，`HOME=$(mktemp -d)` 跑 install.sh 验首装路径，再跑一次验升级路径（匿名下载仍需仓库转 public）

## Windows 收尾

- [ ] 修绿 windows-e2e 全量套件：release.yml 的 windows-e2e job 跑全 workspace nextest，v1.11.3 首发曝光失败（fixture 写死 `/bin/sh`、`/tmp` 等 unix 路径；fail-fast 掩盖完整失败面，需 `--no-fail-fast` 拿全清单后分批平台化 fixture，本机无 Windows 只能 CI 迭代）；修绿后把 `release.needs` 的 `windows-e2e` 加回（v1.11.3 为发版临时摘除）

PM3-59 已合入并发版（v1.11.0）：`pm3 service install/uninstall/status` 与 `pm3 install` 换代链在 Windows 走 Task Scheduler + 命名管道，能力矩阵见 `docs/windows.md`。剩余验收项：

- [ ] Windows 真机验收：`service install --dry-run` 目检 → 注册/`/Query` → start/list/logs/stop → 注销重登自启 → `pm3 install` 换代 → uninstall 清场（`frameworks/tests/service_windows.rs` 已备好同路径 e2e；release.yml 已有 windows-latest job 在 tag 时自动跑这份 e2e）
  - 重点疑点：`pm3 install` 默认落位 `~\bin\pm3` 无 `.exe` 后缀（`adapters/src/install/layout.rs` 的 `DEFAULT_DESTINATION`），Windows 上无扩展名不可执行，验收时确认并修
- [ ] schtasks 非英文 locale 下 `/Query` 输出解析失效（状态恒报 not running），需要时换 PowerShell `Get-ScheduledTask` 的对象化输出
