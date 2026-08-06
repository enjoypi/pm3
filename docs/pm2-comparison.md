# pm3 对照 pm2 的优劣

对照对象：Node 版 pm2 v7.0.3。目的是分清**哪些缺口是设计取舍、哪些是真的该补**。

规模基准：pm2 `lib/` 约 20871 行 JS（不含 `modules/`、`test/`）；pm3 生产 11102 行 Rust + 测试 22197 行（1572 个测试函数，测试:生产 ≈ 2:1）。

## 结论

pm3 在**安全边界、状态正确性、工程质量**三个维度显著优于 pm2；在**多实例/零停机/可观测性/生态**四个维度全面缺席。它不是 pm2 的替代品，而是「单机、少量服务、要求强隔离」这一细分场景下更可靠的实现。

## pm3 的优势

| 维度 | pm3 | pm2 |
|---|---|---|
| **进程隔离** | seatbelt/bwrap 沙箱，默认只写自己工作目录、默认拒网，可写根逐条声明 | 无沙箱/cgroup/rlimit/seccomp，唯一隔离是 `uid`/`gid` 且要 root |
| **凭据处理** | 只走 `<name>.env`，读后 `chmod 0600`；yaml 写 `env` 直接报错；不进 describe / dump / 任何日志；`AppSpec` 无 `Serialize`，三处全字段解构防泄漏 | secret 明文进 `pm2_env` → 写入 `dump.pm2`、`pm2 describe`/`jlist`/`env`、RPC 响应、HTTP `GET /`；`pm2 report`（给 issue 用）直接打印 daemon 完整 `process.env` |
| **控制面鉴权** | UDS `chmod 0600`，不占任何网络端口 | RPC socket `chmod 775`（同组用户可调 `prepare` 启动任意二进制，无认证）；`pm2-runtime --web` 的 HTTP 无鉴权 + CORS 全开 + 默认吐全部 env |
| **pid 复用防护** | 三元组指纹（`ps lstart` token + 启动参数 SHA256 + 二进制 SHA256），三态 `Liveness{Alive,Gone,Unreadable}`；kill 前复验 token（`sweep_pid`）；`is_signalable` 挡住 procps 对超 i32 值的静默截断 | 只有 `process.kill(pid,0)` 存在性检查；TreeKill 的 `ps` 快照与实际 kill 之间存在竞态，`kill_timeout` 窗口内可误杀复用 pid |
| **daemon 换代** | 逐服务核对指纹 → `Adopt`/`Settle`/`Respawn`；读不出声明的存活进程走 `sweep_stranded` 停掉并记日志 | `resurrect` 完全不读 pid 文件、不接管旧进程，只按 name 去重 → daemon 崩溃后旧进程变孤儿，恢复时再拉一份可能双跑 |
| **依赖编排** | `DepGraph` 拓扑排序 + 环检测（`Cycle{involved}`），start 与 resurrect 都按序；被依赖时 `delete` 返回 409 | 无 `depends_on` 概念，`_startJson` 并发度 2 乱序启动，`resurrect` 同样并发 |
| **状态语义** | `timers` 同时是 `next` 列数据源与调度标记 ⇒ `next` 空 ⇔ 真停了 | `pm2 stop` 不注销 cron，只打 warning（`API.js:1405`），出现「服务已停但定时器仍在」 |
| **熔断正确性** | `decide_restart` 纯函数，跑满 `min_uptime` 即清零 | 熔断判定窗口是 `min_uptime * max_restarts`（默认 16s），**超过这个总时长后 `unstable_restarts` 永不增长**（`God.js:455`）⇒ 长期反复崩溃的进程无限重启 |
| **配置可移植** | 服务文件无绝对路径（`${HOME}` / `${PM3_SERVICE_CWD}` 折叠）；意图（`~/.config/pm3`）与运行状态（`~/.pm3`）分离；写入冲突打 diff 拒绝，`--force` 才覆盖 | ecosystem 里绝对路径随手写；`dump.pm2` 混装意图与状态；`.config.js` 直接 `require()`，JSON 走 `vm.runInThisContext`（共享全局，不是沙箱） |
| **工程质量** | 四层 Clean Architecture + `arch_tests` 强制依赖方向；clippy 四组全开 `-D warnings`；`just cov` 四指标 100% + lcov 真值 plate（防「不编译就 100%」） | 无类型（`types/index.d.ts` 明显滞后，缺 `scale`/`deploy`/`link` 等）；已知 bug 如 `ProcessContainer.js:84` 判 `process.env.gid` 却设 `pm2_env.gid`，cluster 降权窗口不干净 |
| **部署形态** | 单静态二进制，运行时只依赖 `/bin/ps`、`/bin/kill` | 需要 Node 运行时 + npm 依赖树（axon/blessed/chokidar/pidusage/…） |
| **cron 表达力** | OpenBSD 随机语法 `~` / `a~b` / `a~b/n`，每次触发重新摇号（错峰） | 只有标准 cron（croner） |

## pm3 的劣势

### 硬缺口（pm2 有、场景常用、pm3 完全没有）

1. **无 cluster / `instances` / `scale` / 负载均衡** —— Node web 服务吃不满多核。pm2 靠 Node 内置 cluster 的 SO_REUSEPORT。
2. **无零停机 reload** —— pm3 的 `restart` 是 stop→start，必有中断窗口；`reload_declaration` 只是重读配置。pm2 有 hard/soft reload（但仅 cluster 模式生效，fork 模式同样退化成 restart）。
3. **无就绪探针** —— 没有 `wait_ready`/`listen_timeout` 等价物。**这是 `DepGraph` 的配套缺口**：依赖顺序只保证 spawn 先后，不保证被依赖者真的可服务。
4. **无内存熔断** —— 无 `max_memory_restart`，内存泄漏无兜底。pm2 有，但 30s 轮询粒度，突发 OOM 也兜不住。
5. **无任何资源指标** —— `ps` 只取 `pid=,lstart=` 两列，无 CPU/RSS 采集，`list` 看不到资源占用，无 `monit` 面板。
6. **无 watch 热重载** —— 开发态不可用（pm2 有 `--watch` 与 `pm2-dev`）。
7. **无 programmatic API / 事件总线** —— 集成面只有 UDS 上 6 条 HTTP 路由；pm2 有 `pm2.connect()`、27 个 RPC 方法、pub/sub bus。
8. **无 deploy / 模块系统 / APM 生态** —— pm2 有 `pm2 deploy`、`pm2 install <module>`、pm2.io 上报、OpenTelemetry。

### 软缺口（低成本可补，当前确实硌手）

9. **stderr 无 CLI 入口** —— `-err.log` 有落盘（`usecases/src/log_paths.rs:2`），但 `frameworks/src/commands.rs:261` 只暴露 `stdout_log`，看错误日志只能自己 `cat`。
10. **无 JSON 输出** —— 没有 `jlist` 等价物，`list`/`describe` 只有人类可读表格，脚本与监控无法消费。
11. **无多服务聚合 tail** —— `pm3 logs` 只能盯一个服务；pm2 `pm2 logs` 可聚合。
12. **无日志写侧 rotate、无 `flush`** —— 读侧兼容外部 rotate（inode 变化重开），但自己不切割也不清空。
13. **无指数退避** —— `restart_delay_ms` 固定值；pm2 有 `exp_backoff_restart_delay`（上限硬编码 15s）。
14. **无手动重置熔断计数** —— 无 `pm2 reset` 等价物。影响不大（跑满 `min_uptime` 自动清零），但 errored 后立刻 restart 再快速崩溃会当场再次熔断。

### 结构性限制

15. **平台面窄** —— 只支持 macOS launchd + Linux systemd **user** 级；pm2 覆盖 systemd/upstart/systemv/openrc/launchd/rcd/smf 共 8 种，另有 Windows 支持。pm3 无 Windows。
16. **沙箱在关键场景失效** —— seatbelt 不允许嵌套，自带沙箱的程序（sshd、macOS app bundle）必须配 `danger-full-access`，安全卖点在这些服务上归零。
17. **单机单用户** —— 不占端口的设计同时排除了远程管理与多机（这是明确的设计取舍，非缺陷）。
18. **macOS 覆盖率门禁本就挂 3 处**（`TODO.md`）—— `host_uid`、`ps_probe.rs`、`watcher.rs` 三处平台性基线缺口，判断回归要逐文件对比而非看退出码。

## 不算优劣的设计取舍

不占任何网络端口、不接受 yaml `env` 字段、不继承启动 shell 环境、不做 `~name` 展开、不静默覆盖配置 —— 均为 `docs/requirements.md` 明示的边界，是 pm3 的定位而非遗漏。

## 若要补齐：建议优先级

### P0（低成本、不破坏设计目标，建议做）

| 项 | 落点 |
|---|---|
| stderr 查看入口 `pm3 logs --err` / `--all` | `frameworks/src/commands.rs` 复用 `usecases::log_paths::STDERR_SUFFIX`，与 `stdout_log` 同一校验路径 |
| `--json` 输出（`list` / `describe`） | `adapters/src/presenter/` 加一个 JSON presenter，与 `table.rs`/`describe.rs` 并列；MUST 沿用「不含 env」的字段集 |
| 内存熔断 `max_memory_rss` | `adapters/src/process/ps_probe.rs` 的 `BATCH_FORMAT` 加 `rss=` 一列，判定抽成 `entities` 纯函数，复用 `watcher.rs` 现有轮询；同步 `AppEntry`/`AppSpec`/指纹与各层 fixture |

### P1（有价值但改动面大）

- 就绪探针（`ready_probe`: exec/tcp + `listen_timeout`），补 `DepGraph` 的语义缺口 —— 判定逻辑放 `usecases`，探测实现放 `adapters`
- 指数退避（扩 `entities/src/process/restart.rs::decide_restart`，纯函数，测试成本低）
- 日志写侧 rotate（size 触发，读侧 `LogFollower::reopen_if_rotated` 已兼容）

### P2（与「极简 + 单机 + 强隔离」定位冲突，建议明确不做）

cluster / `scale` / 零停机 reload（需要端口共享或 socket 传递，与沙箱模型冲突）、watch 热重载、deploy、模块系统、programmatic API、APM 上报。
