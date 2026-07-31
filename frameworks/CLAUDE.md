# frameworks — 入口与组装

`main.rs` + DI 组装 + 路由绑定 + 日志初始化 + 生命周期。不放业务逻辑、不放格式转换。

**MUST NOT 依赖 `usecases`/`entities`（`arch_tests` 强制）：内层类型一律从 `adapters` 的具名再导出取。**

## 文件地图

| 文件 | 内容 |
|---|---|
| `main.rs` | 只调 `frameworks::cli`，无重复 mod 编译 |
| `cli.rs` / `commands.rs` | clap 定义与子命令分发 |
| `daemon/` | `bootstrap` `actor`（事件循环 + `timers`）`socket` `service` `ports` |
| `client/uds.rs` | CLI 侧 Unix socket 客户端（`ask` / `ask_report`） |
| `svc.rs` | `cfg_dir/<name>.yaml` 的读写、`reconcile`、`fold_entry`、`SvcUndo` |
| `service.rs` | `pm3 service install/uninstall` |
| `signal.rs` | SIGINT 吞掉、SIGTERM 落盘退出 |
| `layout.rs` / `telemetry.rs` / `prompt.rs` / `sandbox_probe.rs` | 路径布局、日志、交互询问、沙箱可用性探测 |

## 本层规则

### CLI

- `main() -> Result<()>` 会用 **Debug** 打印错误（`Error: SvcConflict {..}`）→ MUST 用 `main() -> ExitCode` + 显式 `eprintln!("{error}")`
- 全局默认值 MUST NOT 在 `execute()` 里现算，交给 clap `default_value_t` 在构建期算
  原因：e2e 的假进程是 `pm3 __sleep`，子进程环境被 `env_clear()` 清空后没有 `HOME`；「所有子命令都先解析配置路径」会让 sleeper 一启动就退出（症状：e2e 里 app 显示 `stopped ↺1`）。不读配置的命令自然不受影响
- 早期校验 MUST 用 `pm3.search_path` 而非 `std::env::var("PATH")`

### 服务文件与回滚

- `start` 被 daemon 拒绝时 MUST 回滚已写的 `cfg_dir/<name>.yaml`（`svc::SvcUndo` 记前态：原本不存在→删、原本存在→写回）
- 写盘 MUST NOT 挪到 `ask` 之后——daemon 落 `dump.yaml` 时服务文件必须已在
- 写 `~/.pm3/config.yaml`（`service install` 那份）与写 `cfg_dir/<name>.yaml` 共用同一个 `svc::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖。新增「写配置」的路径 MUST 走它，别另起一套

### daemon 收尾

- `clear_runtime_files` MUST 先删 `pm3.pid` 再删 socket：`pm3 kill` 与 e2e 都以「socket 消失」判定 daemon 收尾完成，反序会让「socket 已没、pid 文件还在」被观测到（症状：`signal_semantics` e2e 约 25% 概率挂在 pid 文件断言）

### e2e（`tests/`）

每个 e2e 用独立 `PM3_HOME` tempdir。既有覆盖面：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写 / cwd 外被拒 / 网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 `resurrect`、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。新增端到端行为时先看这里有没有现成场景可挂。

### 覆盖率（本层最容易踩）

- 「只经真实 binary 驱动」的函数 MUST NOT 再补 lib 侧单测
  原因：lib 测试会新增一个实例化，函数其余 region 在该实例化里永不可达 → 门禁挂（症状：加测试反而多出 missed region，`--show-missing-lines` 无输出而 lines/branches 均 100%）
  修法：删掉 lib 单测，失败路径也走 e2e（如 `pm3 --config /nonexistent kill`）
  注意：`main.rs` 只调 `frameworks::cli`、无重复 mod 编译，llvm-cov 仍按「lib test 二进制 + pm3 bin」算两份实例化
- 新增错误分支要走真实 binary：e2e 里 `UnixListener::bind(socket)` 起个假 daemon 回 `200` + 非 JSON body，即可驱动 CLI 的解码失败路径（`tests/stale_socket.rs`）；这类假服务器 MUST 用 `while let Ok(..) = accept()` 且 MUST NOT `join()`（只回固定条数会把测试挂住）
