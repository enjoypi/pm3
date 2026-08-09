# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

- [ ] 在 macOS 上跑一次 `just lint` + `just cov --fresh` 复核：四处平台性缺口的修法（`layout.rs` 的 `owner_uid_of`、`ps_probe.rs` 的重探测试、`watcher.rs` 的自计数 fake ps、`private_file.rs` 的 `fill`）都是在 Linux 上验证的，两平台应同为 100%

## 发布与用户安装方案

现状：README 已写三条安装路径——`install.sh` 一行装（curl|sh，sha256 校验后自动 `pm3 install`）、GitHub Releases 手动下载（产物带 `config.yaml` 与 `.sha256`）、`cargo install --git <url> --bin pm3 --locked`（已实测 virtual workspace 下 `--bin` 能定位）。**前提：仓库转 public**，私有状态下匿名下载全部 404。

- [ ] **`cargo install`（crates.io，暂缓：已拍板不改名先不发布）**：要求**包名**就是用户敲的名字，而现在带 `[[bin]] pm3` 的包叫 `frameworks` ⇒ `cargo install pm3` 装不了。四个 crate 得改名（`pm3` + `pm3-entities` / `pm3-usecases` / `pm3-adapters`）并**全部发到 crates.io**——path 依赖不能发布，少一个都装不上。`entities`/`adapters` 这类通用名在 crates.io 上本就占不到，改名是必经之路
  - MUST NOT 用「合并成单 crate」绕开：`arch_tests` 的依赖方向强制是**靠 crate 边界**成立的，合并等于把它拆了
  - 还缺 crates.io 必需元数据：`license`（或 `license-file`）、`description`、`repository`、`readme`，四个 `Cargo.toml` 现在一个都没有
  - `dev_scripts/rename.ts` 是改**项目名**的模板脚本，不负责这次的 crate 改名
- [ ] 可选分发面：Homebrew tap、`cargo-binstall`（后者跟着 Releases 产物白拿）
- [ ] install.sh 真机验收：等带 `.sha256` 与 `config.yaml` 的新 release 发出后，`HOME=$(mktemp -d)` 跑 install.sh 验首装路径，再跑一次验升级路径

## Windows 收尾

PM3-59 已合入并发版（v1.11.0）：`pm3 service install/uninstall/status` 与 `pm3 install` 换代链在 Windows 走 Task Scheduler + 命名管道，能力矩阵见 `docs/windows.md`。剩余验收项：

- [ ] Windows 真机验收：`service install --dry-run` 目检 → 注册/`/Query` → start/list/logs/stop → 注销重登自启 → `pm3 install` 换代 → uninstall 清场（`frameworks/tests/service_windows.rs` 已备好同路径 e2e；release.yml 已有 windows-latest job 在 tag 时自动跑这份 e2e）
  - 重点疑点：`pm3 install` 默认落位 `~\bin\pm3` 无 `.exe` 后缀（`adapters/src/install/layout.rs` 的 `DEFAULT_DESTINATION`），Windows 上无扩展名不可执行，验收时确认并修
  - 验收通过后：README 安装节补 Windows 手动安装说明（Releases 的 `pm3-<版本>-x86_64-pc-windows-msvc.zip`，产物链已进 release.yml 的 build-windows job）
- [ ] schtasks 非英文 locale 下 `/Query` 输出解析失效（状态恒报 not running），需要时换 PowerShell `Get-ScheduledTask` 的对象化输出
