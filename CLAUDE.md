@~/.claude/CLAUDE.md

@~/.claude/clean-architecture/CLAUDE.md

@~/.claude/rust/CLAUDE.md

pm3: a minimal pm2 with strict sandbox isolation. One binary is both the CLI and the long-running daemon, talking over a unix socket (a named pipe on Windows). CLAUDE.md records only pitfalls; cross-layer ones live here, single-layer ones sink into each crate's own CLAUDE.md.

## 全局优先原则

1. 文档与沟通精简无歧义、人工可维护
2. 使用简体中文（全角标点），不用繁体中文、日文、韩文。文件名用英文
3. 给用户的选项带数字编号，第 1 项为推荐或 All
4. 批量改动用脚本或工具完成；脚本用 Bun + TypeScript
5. SOLID：一个类/模块/函数只负责一项功能；针对接口编程；拆分胖接口；用 Interface/Trait 而非继承
6. DRY：同一逻辑只保留一份，抽取为函数/模块
7. YAGNI：只写当前需要的功能；依次优先复用仓内代码、stdlib、平台原生能力、已装依赖，都不行才写能跑通的最小实现
8. 代码文件（含测试）不超过 512 行、函数不超过 64 行，超限即拆分
9. 不写注释或 docstring，删除已有注释
10. 存储与日志时间用 UTC；面向人展示时标注时区（如 2026-07-17 09:20:09 UTC+8）并支持 TZ 环境变量
11. 新配置文件用 yaml；默认值、数值常量、业务字符串常量从配置读取
12. secret 只经环境变量加载、不设默认值，写法 db_password: "${DB_PASSWORD}"
13. 输出 debug 级结构化日志（request_id、feature、user、action、req、resp 等），供 AI 分析
14. 测试先行：先写失败的测试再实现；覆盖率先保证 branch 100% 再保证 region 100%；Web 前端用 happy-dom 断言 DOM 与数据，不截图
15. 测试与业务代码分文件
16. 不自行新建 git branch 或 worktree
17. 多轮改动合并为一个 git commit；commit message 单行、少于 64 个 UTF-8 字符、不含 Co-Authored-By 或 Claude 签名，格式 [TASK-ID ]type: 业务描述，type ∈ build/chore/ci/docs/feat/fix/perf/refactor/revert/style/test，TASK-ID 由用户提供或取自最近 git log，没有则留空
18. 可复用的规则、原则、坑写进对应领域的 CLAUDE.md（与既有规则去重合并，单个 CLAUDE.md 少于 256 行），不记入 TODO.md 或 MEMORY.md；一次性规则丢弃
19. TODO.md 是任务清单，任务完成即删除
20. Plan 用 subagent 从至少 3 个角度（含 1 个批判角度）推敲
21. Plan 定稿后把 ~/.claude/plans/<name>.md 移到当前项目目录，无需确认。Plan 完成后删除。
22. 新建文档（.md .rst .adoc .txt 等）先征得用户同意

## Layout and runtime files

- **macOS and Linux keep the identical XDG layout on purpose** (one layout = one test suite = one migration doc); the platform difference is absorbed by probing whether `/run/user/<uid>` exists, never by branching on the operating system. MUST NOT switch macOS to native `~/Library/...`
- **MUST NOT derive any pm3 path from macOS's `$TMPDIR`**: a launchd-started daemon has it and a shell-started CLI does not ⇒ the two compute different socket paths ⇒ the CLI cannot connect, `ensure_daemon_running` spawns a second unmanaged daemon, and two supervisors write one `dump.yaml`
- **The socket and the pid file MUST carry the sticky bit** (`adapters::write_sweep_proof` / `SWEEP_PROOF_FILE`, 0o1600): the XDG spec lets `XDG_RUNTIME_DIR` be swept by atime, and nothing in pm3's periodic ticks touches the socket ⇒ an idle daemon's socket disappears while the daemon still holds the unlinked inode ⇒ the next CLI command gets NotFound and spawns a competitor. Sticky beats a "touch it every few hours" tick: no new timer, no new coverage region. Its side effect is exactly the wanted one — neither file survives a reboot, which is what `PidTrust` assumes
- `$XDG_*` is read **once**, at `pm3 startup`, and frozen into the unit as absolute `PM3_{CONFIG,STATE,RUNTIME,DATA}_DIR`: `host_pm3_env()` only filters the `PM3_` prefix, so a launchd/systemd daemon never sees `XDG_STATE_HOME` while the operator's shell does. pm3 internally honours `PM3_*` only

## Diagnostics

- `unstable_restarts` (what the breaker judges) is recomputed only when a process exits, so a service that has been healthy for hours keeps a stale tally. The liveness tick clears it through `usecases::query::settle_stability` once the record has been up past its `min_uptime_ms`. That function MUST stay a **non-generic** function in `query.rs`: written inside `Supervisor::on_*` its regions split across the `impl Ports` instantiations and no amount of tests fills them
- `pm_id` starts at **1** and reuses the gaps deleted services leave (`ProcessTable::lowest_free_pm_id`). `ProcessTable` therefore keeps no `next_pm_id` cursor — the records are the only source of truth, so a sparse `dump.yaml` cannot push new ids to the far end
- 异常通知（`pm3.notify`）缺省即启用 ⇒ 测试用 config MUST 显式写 `notify` 段：lib fixture 写 `enabled: false`，e2e 把两个 program 指向 home 下的替身 `notifier`（不存在时只记 warn），否则跑测试会真弹桌面通知
- liveness watching MUST skip `autorestart: false` services: the failure path is stop → breaker → `GiveUp` → `Errored`, so watching a service the operator asked pm3 not to heal only kills it

## 覆盖率

- 门禁是 `just cov`（`dev_scripts/cov.ts`）：非测试代码先 line 与 branch MUST 100%，再 region MUST 100%；有 line/branch 缺口时只报它们
- MUST NOT 用 `#[coverage(off)]`/`coverage_nightly`、nightly `#![feature]`、成员 crate 的 `[features]`：覆盖不到的代码补测试或删掉；发布用 stable，`[features]` 让一份代码分叉成多条编译路径，门禁只看得到其中一条。由 `arch_tests` 扫源码兜底
- `rust-toolchain.toml` 把仓内默认工具链定为 nightly（含 rust-analyzer 与裸 cargo）；`--release` 只用于 release CI 与 `just install`，二者 MUST 写 `cargo +stable`：toolchain 文件压过 rustup 默认与 `dtolnay/rust-toolchain@stable`
- 插桩靠 cargo-llvm-cov 的 `RUSTC_WRAPPER` 注入，参数不进 cargo 指纹 ⇒ `target/llvm-cov-target` MUST 只经 `cargo_invocation.ts --cov` 写入（`just build/test/cov`），裸 cargo 写入的未插桩产物会被当 Fresh 复用 ⇒ 文件从报告消失、门禁误判通过；`cov.ts` 以「4 个业务 crate 都在报告里且 branch 计数 > 0」兜底，已混入时用 `bun dev_scripts/cargo_invocation.ts --cov llvm-cov clean --workspace` 恢复
- `cov.ts` 只清 profraw、不清产物以复用增量编译；`report` 会扫进同名旧哈希二进制（nightly 升级、Cargo.lock 变化后残留）⇒ 跑完测试按 `--message-format=json` 的本次 executable 删除旧哈希
- 测试跑在 cargo-nextest 上（每个测试独立进程），门禁耗时比 `cargo test` 减半
- macOS 刚 fork 的子进程 `pti_priority` 会先读到 31，约百毫秒内才落到夹紧后的档位 ⇒ 断言优先级 MUST 轮询到稳定值（`priority.rs` 的 `settled_priority`），不能只读一次
- `just build` 产物带插桩，手动运行会在 cwd 写 `default.profraw`；要跑真机用 `just install`
- tracing 的 `log` 特性 MUST 保持关闭：开启后每个日志宏多展开一份「无订阅者时转发给 log crate」的分支，复用同一批字面量且永不执行 ⇒ 所有日志参数行恒为 0。它由 axum 默认特性 `tower-log` 引入，故 axum 关默认特性并显式列出其余特性
- 日志宏只在已启用时才求值参数 ⇒ 各 lib 测试二进制 MUST 经 `ctor` 装 `test_trace`（全级别、只求值不输出），e2e 的 config MUST 用 `log_level: "trace"`
- 同一函数在 lib 测试二进制与 `pm3` 二进制各有一份覆盖记录：line 与 branch 按合并计，region 只取覆盖最多的那一份 ⇒ 两份互补不算数，补测试要让同一份覆盖全部 region（`?` 的错误返回最常漏，通常补 lib 单测）；只有单测能走到、生产路径不可达的分支 MUST 删掉，而不是补单测

## 构建环境

- MUST NOT 在本机用 `--target x86_64-unknown-linux-gnu` 编译或 clippy：unix 侧只在 macOS/Linux 真机上验证
- macOS：Xcode 大版本升级后 `IDEXcodeVersionForAgreedToGMLicense` 作废，`cc` 以 exit 69 拒绝链接。症状会伪装成「依赖更新引入了坏 crate」——`cargo build --all-targets` 只编 lib/test 时不链接，一路绿灯，直到编 bin 或某个依赖带 build script/cdylib 才炸。pm3 只要 linker 与 macOS SDK，Xcode 的其余部分都不需要 ⇒ 用 `DEVELOPER_DIR=/Library/Developer/CommandLineTools`（免 sudo、只影响当次）或 `sudo xcode-select -s /Library/Developer/CommandLineTools`（永久）。`sudo xcodebuild -license accept` 也非交互，但每次 Xcode 升级都要重跑
- `just install` MUST 自行解析真机 config（`PM3_CONFIG_DIR` → `$XDG_CONFIG_HOME/pm3` → `~/.pm3`），MUST NOT 硬写 `--config config.yaml`：仓内那份的 roots 全为空，装上去会让机器直接跳到推导出的 XDG 布局，而 `dump.yaml` 与服务文件还在旧位置 ⇒ 下一个 daemon rejoin 不到任何东西，所有在跑的服务变成孤儿。三处都找不到才视为全新机器，由 `pm3 install` 在 XDG 路径写入内嵌的默认 config（`adapters::DEFAULT_CONFIG`）
- Linux：Ubuntu 24.04+ 默认 `kernel.apparmor_restrict_unprivileged_userns=1`，只装 bubblewrap 不够，bwrap 报 `setting up uid map: Permission denied` ⇒ 被管服务起不来，沙箱 e2e 只表现为「日志里等不到 done」。MUST 给 `/usr/bin/bwrap` 加带 `userns,` 的 AppArmor profile（`/etc/apparmor.d/bwrap`，`flags=(unconfined)`）并 `apparmor_parser -r`
- macOS：ld 自动加的 ad-hoc 签名在 TCC 眼里按 cdhash 记账 ⇒ 任何 rebuild 后 `just install` 都静默吊销既有授权（含完全磁盘访问，设置面板勾选仍在但对新二进制无效），被管服务访问云盘/文档目录时弹窗反复出现且归账到 daemon（responsible process，如 gdrive 服务跑 Google Drive 本体 ⇒ 弹窗挂在「pm3」头上）⇒ `just install` 与 release CI MUST 用同一把 `pm3-local` 自签证书（`just signing-identity` 生成并导入 login keychain，CI 侧配 secrets `PM3_CODESIGN_P12`/`PM3_CODESIGN_P12_PASSWORD`）把 binary 签成稳定身份，TCC 改按证书记账，跨重装稳定；本机与 CI 各生成一把同名证书 = 两个身份，授权照样失效
- macOS `self_responsible`：负责进程在进程诞生时定死，后代继承 ⇒ 跳板 `pm3 __disclaim` MUST 在 argv 最外层并用 `POSIX_SPAWN_SETEXEC`（pid、`ps lstart` 身份不变），故只许 `danger-full-access`（受限服务最外层是 `sandbox-exec`）。exec 成功后跳板进程的 profraw 不落盘 ⇒ `disclaim::exec_disclaimed` MUST 保持无分支直线代码，由失败用例覆盖全部行。该字段不进启动指纹：改它不该让 daemon 重启时连带重启 sshd 断开会话。tmux server 归属取决于首个启动它的会话。服务的 `script` MUST 直接写需要授权的程序，MUST NOT 用 `sh -c 'exec …'` 包一层：TCC 按 disclaim 时被启动的程序记账（同系统 sshd 要给 `sshd-keygen-wrapper` 授权），包一层就得给 `/bin/sh` 授权，所有 sh 脚本跟着获权
- macOS 异常通知：`osascript` 发出的通知归账到「脚本编辑器」，且不论是否显示都返回 0 ⇒ daemon 日志里的 `action: notify` 成功不代表用户看到了。真机 MUST 先在「系统设置 → 通知 → 脚本编辑器」允许通知并把「显示预览」设为「始终」：未授权时静默丢弃，预览为「默认」且全局不是「始终」时只弹出不带正文的空通知
- macOS `priority`：
  - 机制：apptype 与 QoS 夹紧在 spawn 时设定并遗传给全部后代 ⇒ macOS 上每个服务都经跳板 `pm3 __disclaim --disclaim <0|1> --process-type <N> --qos-clamp <N>` 启动，三个 setter 无条件调用，0 表示继承
  - 默认值：clap 默认 `--disclaim 1`、其余为 0，MUST 保持，使旧形态 `__disclaim -- prog` 语义不变。原因：`just install` 换完二进制、交接之前，旧 daemon 会用新二进制拉起服务
  - plist 的 `ProcessType` MUST 为 `Interactive`：
    - `Standard` 会被 launchd 加 utility 夹紧，子进程无法解除，任何服务都到不了 interactive
    - `Background` 会让 daemon 整体落到 pri 4
  - daemon 自降 MUST 只用线程 QoS（`runtime_for` 的 `on_thread_start`）：线程 QoS 不遗传，夹紧与 darwinbg 会遗传给服务
  - 跳板让 spawn 本身恒成功 ⇒ `disclaim::wrap` 先同步拒绝不可执行的 `program`，保住「批量启动半成功即回滚」；ENOEXEC 仍只能由跳板写入服务的错误日志
  - lib 单测里 `current_exe` 是测试二进制，跳板由 `platform::trampoline_program` 的 sh 替身顶替
  - 观测：读 `proc_pidinfo` 的 `pti_priority`/`pth_priority`（`getpriority` 恒为 0）；e2e MUST NOT 用 `taskpolicy` 模拟后台父进程（它设的 ext darwinbg 无法解除）
- `windows-e2e` 不在 release 的 `needs` 里 ⇒ 红叉不影响产物。CI runner 是管理员 ⇒ 只有非管理员才触发的权限缺陷（如 schtasks 缺 `UserId`）在 CI 上永远绿，MUST 在本机非管理员账户跑一遍才算验证
- Windows：Bash hook 的 `env -i` 会剥掉 `PROGRAMDATA`/`TMP`/`TEMP`/`USERNAME` ⇒ rustc 找不到 VS，回退到 Git 自带的 GNU `link`（报 `extra operand`）；`link.exe` 把临时文件写到 `C:\WINDOWS`（LNK1104）；schtasks 的 e2e 拿不到账户 ⇒ cargo 命令 MUST 显式传入这四个变量。rustup default host 为 gnu 时，`rust-toolchain.toml` 的 nightly 会解析到 gnu 工具链 ⇒ MUST `rustup set default-host x86_64-pc-windows-msvc`
- Windows 覆盖率：`std::process::exit`（含 clap 的 `--version`/`--help`/解析失败）直接 `ExitProcess`，不跑 CRT atexit ⇒ 子进程 profraw 不落盘，这些路径在 Windows 门禁里恒为 0 命中；只有 `main` 返回 `ExitCode` 的路径计入。`frameworks/tests/` 25 个 e2e 里 24 个是 `cfg(unix)` ⇒ Windows 的 frameworks 覆盖率只能靠逐个移植 e2e 补
- `target/llvm-cov-target` 出现 `os error 1392`（文件或目录损坏）或 `ls` 报 `File exists` 时，门禁数字不可信（子进程计数丢失），MUST 先 `mv` 走该目录重跑，再对该盘跑 `chkdsk`
