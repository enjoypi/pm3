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

## 构建环境

- macOS：Xcode 大版本升级后 `IDEXcodeVersionForAgreedToGMLicense` 作废，`cc` 以 exit 69 拒绝链接。症状会伪装成「依赖更新引入了坏 crate」——`cargo build --all-targets` 只编 lib/test 时不链接，一路绿灯，直到编 bin 或某个依赖带 build script/cdylib 才炸。pm3 只要 linker 与 macOS SDK，Xcode 的其余部分都不需要 ⇒ 用 `DEVELOPER_DIR=/Library/Developer/CommandLineTools`（免 sudo、只影响当次）或 `sudo xcode-select -s /Library/Developer/CommandLineTools`（永久）。`sudo xcodebuild -license accept` 也非交互，但每次 Xcode 升级都要重跑
- `just install` MUST 自行解析真机 config（`PM3_CONFIG_DIR` → `$XDG_CONFIG_HOME/pm3` → `~/.pm3`），找不到就失败：recipe 原先硬写 `--config config.yaml`，而仓内那份的 roots 全为空，装上去会让机器直接跳到推导出的 XDG 布局，而 `dump.yaml` 与服务文件还在旧位置 ⇒ 下一个 daemon rejoin 不到任何东西，所有在跑的服务变成孤儿
- macOS：ld 自动加的 ad-hoc 签名在 TCC 眼里按 cdhash 记账 ⇒ 任何 rebuild 后 `just install` 都静默吊销既有授权（含完全磁盘访问，设置面板勾选仍在但对新二进制无效），被管服务访问云盘/文档目录时弹窗反复出现且归账到 daemon（responsible process，如 gdrive 服务跑 Google Drive 本体 ⇒ 弹窗挂在「pm3」头上）⇒ `just install` 与 release CI MUST 用同一把 `pm3-local` 自签证书（`just signing-identity` 生成并导入 login keychain，CI 侧配 secrets `PM3_CODESIGN_P12`/`PM3_CODESIGN_P12_PASSWORD`）把 binary 签成稳定身份，TCC 改按证书记账，跨重装稳定；本机与 CI 各生成一把同名证书 = 两个身份，授权照样失效
- `windows-e2e` 不在 release 的 `needs` 里 ⇒ 红叉不影响产物。CI runner 是管理员 ⇒ 只有非管理员才触发的权限缺陷（如 schtasks 缺 `UserId`）在 CI 上永远绿，MUST 在本机非管理员账户跑一遍才算验证
- Windows：Bash hook 的 `env -i` 会剥掉 `PROGRAMDATA`/`TMP`/`TEMP`/`USERNAME` ⇒ rustc 找不到 VS，回退到 Git 自带的 GNU `link`（报 `extra operand`）；`link.exe` 把临时文件写到 `C:\WINDOWS`（LNK1104）；schtasks 的 e2e 拿不到账户 ⇒ cargo 命令 MUST 显式传入这四个变量。rustup default host 为 gnu 时，`+nightly` 会解析到 gnu 工具链 ⇒ MUST `rustup set default-host x86_64-pc-windows-msvc`
- Windows 覆盖率：`std::process::exit`（含 clap 的 `--version`/`--help`/解析失败）直接 `ExitProcess`，不跑 CRT atexit ⇒ 子进程 profraw 不落盘，这些路径在 Windows 门禁里恒为 0 命中；只有 `main` 返回 `ExitCode` 的路径计入。`frameworks/tests/` 25 个 e2e 里 24 个是 `cfg(unix)` ⇒ Windows 的 frameworks 覆盖率只能靠逐个移植 e2e 补
- `target/llvm-cov-target` 出现 `os error 1392`（文件或目录损坏）或 `ls` 报 `File exists` 时，门禁数字不可信（子进程计数丢失），MUST 先 `mv` 走该目录重跑，再对该盘跑 `chkdsk`
