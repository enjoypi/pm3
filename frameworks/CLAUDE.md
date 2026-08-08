# frameworks — 入口与组装

`main.rs` + DI 组装 + 路由绑定 + 日志初始化 + 生命周期。不放业务逻辑、不放格式转换。

**MUST NOT 依赖 `usecases`/`entities`（`arch_tests` 强制）：内层类型一律从 `adapters` 的具名再导出取。**

## 文件地图

| 文件 | 内容 |
|---|---|
| `main.rs` | 只调 `frameworks::cli`，无重复 mod 编译 |
| `cli.rs` / `commands.rs` | clap 定义与子命令分发 |
| `logs.rs` | `pm3 logs`：读侧聚合（`cfg_dir` 枚举、流选择、行前缀、follow），不经 daemon |
| `daemon/` | `bootstrap` `actor`（事件循环：把事件交给 `adapters::Supervisor`，再把返回的 `SupervisionEffect` 逐条交给 `TaskBoard`）`timers`（`TaskBoard`：纯 `JoinHandle` 表，spawn/abort cron 定时器、待重启任务、强杀延时、退出监听、就绪探针、内存采样与日志切割 tick）`socket` `service` `ports` |
| `client/uds.rs` | CLI 侧 Unix socket 客户端（`ask` / `ask_report`） |
| `server.rs` | `serve_listener`：接管已 bound 的 listener，避开 bind→drop→re-bind 的抢占窗口 |
| `service.rs` | `pm3 service install/uninstall` |
| `install.rs` | `pm3 install`：备份、原子换二进制、换代重装、接管等待、before/after 对比（编排放这里，判定纯函数在 `usecases::handover`，fs/管理器探测在 `adapters::install` 与 `adapters::unit`） |
| `signal.rs` | SIGINT 吞掉、SIGTERM 落盘退出 |
| `layout.rs` / `telemetry.rs` / `prompt.rs` / `sandbox_probe.rs` | 路径布局、日志、交互询问、沙箱可用性探测 |

## 本层规则

### daemon 编排

- 业务判断一律不在本层：`Daemon` 只做「收事件 → 问 `Supervisor` → 派发效果」，新增行为改 `usecases/supervisor.rs`，本层只在 `TaskBoard::apply` 里补一条 spawn/abort
- 本层 MUST NOT 自己给进程发信号：收尾清扫走 `Supervisor::force_kill_survivors`，与 `on_force_kill` 共用 `sweep_pid` 的身份守卫（曾经在 `Daemon::force_kill_survivors` 里裸 `ports.force_kill(pid).await.ok()`，既不校验 token 也不记失败日志，pid 复用后会打掉无关进程组）
- 一种效果 MUST 只有一个执行者：`WatchExit` 的 spawn 也在 `TaskBoard` 里，别让 `Daemon::run` 提前截胡——那会让 `TaskBoard` 的对应 match arm 永不可达，覆盖率补不上（不可达分支应重写消除，不是加测试掩盖）

### CLI

- `main() -> Result<()>` 会用 **Debug** 打印错误（`Error: ServiceConflict {..}`）→ MUST 用 `main() -> ExitCode` + 显式 `eprintln!("{error}")`
- 全局默认值 MUST NOT 在 `execute()` 里现算，交给 clap `default_value_t` 在构建期算
  原因：e2e 的假进程是 `pm3 __sleep`，子进程环境被 `env_clear()` 清空后没有 `HOME`；「所有子命令都先解析配置路径」会让 sleeper 一启动就退出（症状：e2e 里 app 显示 `stopped ↺1`）。不读配置的命令自然不受影响
- CLI 侧日志：`open_session` / `open_service_session` 各调一次 `init_cli_telemetry`（写 **stderr**，不能污染作为报文的 stdout）。MUST NOT 挪到 `dispatch`/`execute` 里——`pm3 __sleep` 不读配置（子进程 `env_clear()` 后没有 `HOME`），`pm3 daemon` 自己装 `LogSink::Stdout` 那份；`try_init` 的重复安装由 `.ok()` 兜住
- 早期校验 MUST 用 `pm3.search_path` 而非 `std::env::var("PATH")`，`sandbox_probe::detect_host_backend` 也是（它 MUST 返回解析后的 `HostSandbox { backend, program }` 绝对路径，不能只回一个 bool：子进程 `env_clear()` 后裸名 `bwrap` 只在 `/bin:/usr/bin` 里找，装在 `/usr/local/bin` 就每次 spawn 报 ENOENT 而探测仍宣称沙箱可用）

### 服务文件与回滚

- `start` 被 daemon 拒绝时 MUST 回滚已写的 `cfg_dir/<name>.yaml`（`adapters::ServiceUndo` 记前态：原本不存在→删、原本存在→写回）。但 daemon 部分成功时 MUST 只回滚 `ReplyDto.refused` 里的服务（`undo.run_for`）——已经在跑的服务不能丢服务文件，详见根 `CLAUDE.md` 的「CLI ↔ daemon 协议」
- 写盘 MUST NOT 挪到 `ask` 之后——daemon 落 `dump.yaml` 时服务文件必须已在
- 写 `~/.pm3/config.yaml`（`service install` 那份）与写 `cfg_dir/<name>.yaml` 共用同一个 `adapters::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖。新增「写配置」的路径 MUST 走它，别另起一套

### daemon 收尾

- `clear_runtime_files` MUST 先删 `pm3.pid` 再删 socket：`pm3 kill` 与 e2e 都以「socket 消失」判定 daemon 收尾完成，反序会让「socket 已没、pid 文件还在」被观测到（症状：`signal_semantics` e2e 约 25% 概率挂在 pid 文件断言）

### e2e（`tests/`）

每个 e2e 用独立 `PM3_HOME` tempdir。既有覆盖面：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写 / cwd 外被拒 / 网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 `resurrect`、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。新增端到端行为时先看这里有没有现成场景可挂。

### 覆盖率（本层最容易踩）

- `src/tests/` 与 `src/test_helpers/` 下的文件名 MUST 以 `_tests.rs` 或 `_test_helpers.rs` 结尾：门禁的 `listProductionSources` 靠这两个后缀排除测试文件，取别的名字会被当成「生产文件缺失于 lcov」而失败
- 会 `panic!` 的测试辅助函数 MUST 放 `src/tests/*_tests.rs`（llvm-cov 忽略路径含 `tests/` 的文件），放 `test_helpers/` 会把 panic 分支算成未覆盖行
- 「只经真实 binary 驱动」的函数 MUST NOT 再补 lib 侧单测
  原因：lib 测试会新增一个实例化，函数其余 region 在该实例化里永不可达 → 门禁挂（症状：加测试反而多出 missed region，`--show-missing-lines` 无输出而 lines/branches 均 100%）
  修法：删掉 lib 单测，失败路径也走 e2e（如 `pm3 --config /nonexistent kill`）
  注意：`main.rs` 只调 `frameworks::cli`、无重复 mod 编译，llvm-cov 仍按「lib test 二进制 + pm3 bin」算两份实例化
- 新增错误分支要走真实 binary：e2e 里 `UnixListener::bind(socket)` 起个假 daemon 回 `200` + 非 JSON body，即可驱动 CLI 的解码失败路径（`tests/stale_socket.rs`）；这类假服务器 MUST 用 `while let Ok(..) = accept()` 且 MUST NOT `join()`（只回固定条数会把测试挂住）
