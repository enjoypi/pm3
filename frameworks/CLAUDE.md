# frameworks — 入口与组装

`main.rs` + DI 组装 + 路由绑定 + 日志初始化 + 生命周期。不放业务逻辑、不放格式转换。

**MUST NOT 依赖 `usecases`/`entities`（`arch_tests` 强制）：内层类型一律从 `adapters` 的具名再导出取。**

## 文件地图

| 文件 | 内容 |
|---|---|
| `main.rs` | 只调 `frameworks::cli`，无重复 mod 编译 |
| `cli.rs` / `commands.rs` | clap 定义与子命令分发 |
| `logs.rs` | `pm3 logs`：读侧聚合（`cfg_dir` 枚举、流选择、行前缀、follow），不经 daemon；细则见下「pm3 logs」 |
| `daemon/` | `bootstrap` `actor`（事件循环：把事件交给 `adapters::Supervisor`，再把返回的 `SupervisionEffect` 逐条交给 `TaskBoard`）`timers`（`TaskBoard`：纯 `JoinHandle` 表，spawn/abort cron 定时器、待重启任务、强杀延时、退出监听、就绪探针、内存采样与日志切割 tick）`socket`（unix 是 `OwnerOnlyListener`，Windows 是 `PipeListener` 命名管道 + `pm3.sock` 存在性标记文件，对外统一 `Pm3Listener` 别名）`service` `ports` |
| `client/uds.rs` | CLI 侧 socket 客户端（`ask` / `ask_report`）；传输按平台分叉：unix 是 `UnixStream`，Windows 是命名管道（`connect_transport` 返回 `Box<dyn Transport>`，HTTP 编解码两平台共用） |
| `server.rs` | `serve_listener`：接管已 bound 的 listener，避开 bind→drop→re-bind 的抢占窗口 |
| `service.rs` | `pm3 service install/uninstall` |
| `install.rs` | `pm3 install`：备份、原子换二进制、换代重装、接管等待、before/after 对比（编排放这里，判定纯函数在 `usecases::handover`，fs/管理器探测在 `adapters::install` 与 `adapters::unit`） |
| `signal.rs` | SIGINT 吞掉、SIGTERM 落盘退出；Windows 同 API 双实现（CTRL_C 吞掉、CTRL_SHUTDOWN 落盘退出） |
| `layout.rs` / `telemetry.rs` / `prompt.rs` / `sandbox_probe.rs` | 路径布局、日志、交互询问、沙箱可用性探测 |

## 本层规则

### daemon 编排

- 业务判断一律不在本层：`Daemon` 只做「收事件 → 问 `Supervisor` → 派发效果」，新增行为改 `usecases/supervisor.rs`，本层只在 `TaskBoard::apply` 里补一条 spawn/abort
- 本层 MUST NOT 自己给进程发信号：收尾清扫走 `Supervisor::force_kill_survivors`，与 `on_force_kill` 共用 `sweep_pid` 的身份守卫（规则见根「进程与信号」）
- 一种效果 MUST 只有一个执行者：`WatchExit` 的 spawn 也在 `TaskBoard` 里，别让 `Daemon::run` 提前截胡——那会让 `TaskBoard` 的对应 match arm 永不可达，覆盖率补不上（不可达分支应重写消除，不是加测试掩盖）

### CLI

- `main() -> Result<()>` 会用 **Debug** 打印错误（`Error: ServiceConflict {..}`）→ MUST 用 `main() -> ExitCode` + 显式 `eprintln!("{error}")`
- 全局默认值 MUST NOT 在 `execute()` 里现算，交给 clap `default_value_t` 在构建期算
  原因：e2e 的假进程是 `pm3 __sleep`，子进程环境被 `env_clear()` 清空后没有 `HOME`；「所有子命令都先解析配置路径」会让 sleeper 一启动就退出（症状：e2e 里 app 显示 `stopped ↺1`）。不读配置的命令自然不受影响
- CLI 侧日志：`open_session` / `open_service_session` 各调一次 `init_cli_telemetry`（写 **stderr**，不能污染作为报文的 stdout）。MUST NOT 挪到 `dispatch`/`execute` 里——`pm3 __sleep` 不读配置（原因见上条），`pm3 daemon` 自己装 `LogSink::Stdout` 那份；`try_init` 的重复安装由 `.ok()` 兜住
- 早期校验 MUST 用 `pm3.search_path` 而非 `std::env::var("PATH")`，`sandbox_probe::detect_host_backend` 也是（它 MUST 返回解析后的 `HostSandbox { backend, program }` 绝对路径，不能只回一个 bool：子进程 `env_clear()` 后裸名 `bwrap` 只在 `/bin:/usr/bin` 里找，装在 `/usr/local/bin` 就每次 spawn 报 ENOENT 而探测仍宣称沙箱可用）

### pm3 logs

服务名从 `cfg_dir` 枚举文件名取 stem（排序、过滤 `.env` 与非 yaml）。单服务输出逐字无前缀；聚合模式每行加 `<name> | ` 前缀，`--all` 的双流分别用 `<name> [out] | ` 与 `<name> [err] | `。聚合模式缺日志文件跳过，单服务模式报错。

### 服务文件与回滚

- `start` 被 daemon 拒绝时 MUST 回滚已写的 `cfg_dir/<name>.yaml`（`adapters::ServiceUndo` 记前态：原本不存在→删、原本存在→写回）。但 daemon 部分成功时 MUST 只回滚 `ReplyDto.refused` 里的服务（`undo.run_for`）——已经在跑的服务不能丢服务文件，详见根 `CLAUDE.md` 的「CLI ↔ daemon 协议」
- 写盘 MUST NOT 挪到 `ask` 之后——daemon 落 `dump.yaml` 时服务文件必须已在
- 写 `~/.pm3/config.yaml`（`service install` 那份）与写 `cfg_dir/<name>.yaml` 共用同一个 `adapters::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖。新增「写配置」的路径 MUST 走它，别另起一套

### daemon 收尾

- `clear_runtime_files` MUST 先删 `pm3.pid` 再删 socket：`pm3 kill` 与 e2e 都以「socket 消失」判定 daemon 收尾完成，反序会让「socket 已没、pid 文件还在」被观测到（症状：`signal_semantics` e2e 约 25% 概率挂在 pid 文件断言）

### e2e（`tests/`）

每个 e2e 用独立 `PM3_HOME` tempdir。既有覆盖面：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写 / cwd 外被拒 / 网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 `resurrect`、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。新增端到端行为时先看这里有没有现成场景可挂。平台门禁走文件第一行的 `#![cfg(unix)]` / `#![cfg(windows)]`（后者目前只有 `service_windows.rs`：dry-run 渲染 + 真 schtasks 注册/卸载）。**MUST NOT 把挂载点改成 `#[cfg(all(test, unix))]`**（对 `src/tests/` 下的单测同样成立）：clippy 的 `tests_outside_test_module` 只认 `#[cfg(test)]` 的 mod，一改挂载点每个测试函数都会报错

技法（集成测试与 e2e 通用）：

- 收尾 MUST 发 SIGTERM 并 wait 到退出：被 SIGKILL 的进程已执行行的计数器永不落盘（`LLVM_PROFILE_FILE` 含 `%p`，子进程各落一份 profraw，但只在正常退出时写出）→ 否则 e2e 覆盖行丢失
- 收尾 helper MUST 无条件「先 `pm3 list` 拉起 daemon 接管、再 `kill --with-services`」；写成「socket 不存在就 return」会漏掉幸存子进程
- 假进程用 `pm3 __sleep <ms>` 隐藏子命令而非 `/bin/sh -c sleep`，摆脱系统 shell 差异；它自身也是生产代码，MUST 有一条「spawn 它、等正常退出、断言退出码 0」的测试
- 测试靶子写 `sh -c` 时 MUST 带 `exec`（`sh -c "exec sleep 30"`）：漏掉时 sh 只 fork 不 exec，信号打在 sh 上、sleep 成孤儿（症状：nextest 报 LEAK、测试卡满整个 sleep 时长）
- `sh -c "trap '' TERM; sleep 30"` 在被 pm3 spawn 时并不能可靠忽略 SIGTERM（手工 shell 与 python spawn 都能，pm3 路径不能，原因未查明）→ 不要用它当「顽固进程」靶子；要覆盖强杀路径就直接调 `on_force_kill`，或先用假的 `on_exit` 让表以为进程已退出
- 断言「依赖先启动」不能看应用自己写的文件（并发写有竞态），要把 `log_level` 调成 debug 后从 `pm3.log` 里读 `"action":"spawn"` 的顺序
- 断言「子进程环境已清空」MUST 探 `$HOME` 不能探 `$PATH`：`/bin/sh` 在 PATH 缺失时会自己合成一个默认值
- 假 daemon（驱动 CLI 的解码失败路径）：`UnixListener::bind(socket)` 后回 `200` + 非 JSON body（`tests/stale_socket.rs`）；MUST 用 `while let Ok(..) = accept()` 且 MUST NOT `join()`——只回固定条数会把测试挂住
- 测「调用外部服务管理器」（`launchctl`/`systemctl`/`loginctl`）用临时目录里的 `#!/bin/sh` 脚本 + `set_permissions(0o755)` 当替身，可同时控制 stdout 与退出码；真实二进制只用 `/usr/bin/true`、`/usr/bin/false`、`/nonexistent/...`，**绝不**在测试里调真的 `launchctl`/`systemctl`
- 断言外部命令的错误文案 MUST 跨平台：合法但不存在的 pid 两边都报 `No such process`，而 `illegal process id` 只有 macOS BSD kill 有；需要「真实存在的程序」的测试用 `/bin/sh`，MUST NOT 写 `/opt/homebrew/...`
- fixture 里的 `create_dir_all` 会把「测试想要它缺失」的父目录造出来 → 造错误路径的 store/source fixture 必须接一个独立 root，别从被测路径 `parent()` 反推
- `#[tokio::test(start_paused = true)]`（测「定时器到点发事件」）需要在 dev-dependencies 显式写 `tokio = { workspace = true, features = ["test-util"] }`——workspace 的 `"full"` **不含** test-util，否则报 `no method named start_paused`；这类测试里 MUST NOT 用带 `timeout` 的 helper 等事件（timeout 也会被自动推进，可能抢先触发），直接 `events.recv().await`
- 交互询问（confirm prompt）的可测模式：循环签名接 `confirm: &mut (dyn FnMut(&str) -> bool + Send)`，生产传一个「每次调用才锁 stdin/stdout」的 fn（`StdinLock` 非 Send，MUST NOT 跨 `.await` 持有），测试传脚本化闭包；MUST NOT 在单测里碰真 stdin（nextest 下 stdin 是 null → 立即 EOF，且无法注入答案）
- 拆超 512 行的测试文件 MUST 把新文件挂成**原测试模块的子 mod**（旧测试文件末尾 `#[path = "x_tests.rs"] mod x;`，新文件开头 `use super::*;`）：子模块能看到父测试模块 `use` 引进来的 helper 与 fixture，挂在生产文件上做兄弟 mod 则看不到

### 覆盖率（本层最容易踩）

- `src/tests/` 与 `src/test_helpers/` 下的文件名 MUST 以 `_tests.rs` 或 `_test_helpers.rs` 结尾：门禁的 `listProductionSources` 靠这两个后缀排除测试文件，取别的名字会被当成「生产文件缺失于 lcov」而失败
- 会 `panic!` 的测试辅助函数 MUST 放 `src/tests/*_tests.rs`（llvm-cov 忽略路径含 `tests/` 的文件），放 `test_helpers/` 会把 panic 分支算成未覆盖行
- 「只经真实 binary 驱动」的函数 MUST NOT 再补 lib 侧单测
  原因：lib 测试会新增一个实例化，函数其余 region 在该实例化里永不可达 → 门禁挂（症状：加测试反而多出 missed region，`--show-missing-lines` 无输出而 lines/branches 均 100%）
  修法：删掉 lib 单测，失败路径也走 e2e（如 `pm3 --config /nonexistent kill`）
  注意：`main.rs` 只调 `frameworks::cli`、无重复 mod 编译，llvm-cov 仍按「lib test 二进制 + pm3 bin」算两份实例化
- 新增错误分支要走真实 binary：e2e 里 `UnixListener::bind(socket)` 起个假 daemon 回 `200` + 非 JSON body，即可驱动 CLI 的解码失败路径（`tests/stale_socket.rs`）；这类假服务器 MUST 用 `while let Ok(..) = accept()` 且 MUST NOT `join()`（只回固定条数会把测试挂住）
