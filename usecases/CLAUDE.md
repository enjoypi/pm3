# usecases — 应用级业务规则

Interactor + Output Port（trait）。与外层交互只经 `ports/` 下的 trait，实现在 `adapters`、注入在 `frameworks`。

## 文件地图

| 文件 | 内容 |
|---|---|
| `supervisor.rs` | `Supervisor`：daemon 全部编排的落点，吃 `SupervisionRequest`/生命周期事件，吐 `(结果, Vec<SupervisionEffect>)` |
| `supervisor_ready.rs` | 就绪探针编排（`on_ready`/`on_ready_timeout`/waiters 级联）：`Supervisor` 的第二个 impl 块，共享字段与方法用 `pub(crate)`，第二 impl 块要 `#[expect(clippy::multiple_inherent_impl)]` |
| `supervision.rs` | 边界契约：`SupervisionRequest` / `SupervisionReply` / `SupervisionFailure` |
| `supervisor_log.rs` | `Supervisor` 的业务日志（`log_*`），MUST 是 `pub`——私有模块内的 `pub(crate)` 触发 clippy `redundant_pub_crate` |
| `timer_state.rs` | `TimerState`：定时器/待重启/generation 的**业务状态**，不持 `JoinHandle` |
| `start.rs` | `start_apps` / `start_one`；`StartMode::{Register, Execute}`；`settle_start`（CLI 侧 start 回复的回滚裁决：refused→Partial、unsaved→Unsaved、否则 Committed） |
| `stop.rs` / `restart.rs` / `delete.rs` | 对应 CLI 动作；`persist_for_handover` 是 daemon 换代收尾（只落盘，不改状态） |
| `resurrect.rs` | daemon 重启后逐服务比对指纹：adopt / evict / respawn |
| `supervise.rs` | 子进程退出与熔断监督循环 |
| `fingerprint.rs` | 身份指纹拼装 |
| `record.rs` / `persist.rs` | 运行态记录与落盘编排 |
| `query.rs` / `table.rs` / `selector.rs` | 查询、列表数据、`AppSelector` 解析 |
| `log_paths.rs` | 日志路径推导 |
| `handover.rs` | `pm3 install` 的 before/after 服务对比：`compare_handover`（adopted/restarted/lost）与 `describe_handover`，纯函数 |
| `ports/` | `clock` `dump_store` `fingerprint` `launcher` `log_rotate` `probe` `ready` `scheduler` `signaler` `wrapper` |

## 本层规则

- 「注册时是否 spawn」的判定 MUST 落在 `start_apps` 的 `StartMode::Register` 分支，**不能**落在执行路径——cron 到点走的是执行路径，判在那里会让定时任务永不运行
- 批处理 Interactor MUST NOT 在循环里 `?`：`start_apps` 返回 `StartReport { outcomes, failure }`（不是 `Result`），`resurrect` 逐服务记 warn 后继续。半路 `?` 会把「已 spawn / 已 adopt」的 outcome 一起丢掉，调用方的 `watch_all` 与末尾的 `save_table` 都不执行 → 进程在跑却没有 watch task，autorestart 与熔断全失效，dump 里也没有它的 pid，daemon 一重启就成永久孤儿
- `resurrect` 的 `topo_sort` 失败 MUST 降级为「按表序恢复」而非中止：`delete` 留下的悬空 `depends_on` 或缺失的服务文件都会让整图排不出序，一 `?` 就把全部幸存进程弃管。`stop_all_apps` 同理（MUST NOT 退回 `unwrap_or_default()`：一个都不停却报成功）
- `Supervisor::restart` 先走 `reload_declaration`（`resolver.prepare` 后覆盖 `record.spec`）再 `restart_app`：显式 `pm3 restart` 要能拾取手改的 `<name>.yaml` 与 `<name>.env`。`on_restart` / `on_fire` / `restart_now` MUST NOT 重读盘——cron 与崩溃自动拉起不该因为文件临时不可读而失败。selector 找不到记录时 `reload_declaration` 返回 `Ok(())`，让 `restart_app` 去报 `NotFound`（错误来源保持唯一）
- `resurrect` 开头 MUST 先 `sweep_stranded`（`stranded` 的来源与 token 守卫见根「环境变量与凭据」）；`surviving_pid` / `evict_pid` 与正常换代路径共用，MUST NOT 复制一份裸 `terminate`
- 强杀只有一条实现 `Supervisor::sweep_pid`（`on_force_kill` 与 `force_kill_survivors` 都走它）；`delete` MUST NOT `forget_generation`，否则 generation 守卫失效，详见根 `CLAUDE.md`「进程与信号」
- `StartReport` 的 `failure`（起不来）与 `unsaved`（起来了但没落盘）MUST 分两个字段携带，协议后果与「`Supervisor::start` 只在 `outcomes` 为空时返回 `Err`」的约定见根「CLI ↔ daemon 协议」
- 新增 Port trait 时不要写 blanket impl：只有 trait 声明的文件进不了 lcov，会触发覆盖率门禁的「生产文件缺失」→ 让实现方显式 `impl Trait for X {}`
- `start_one` / `resurrect` 涉及身份指纹的采集时机与旧进程驱逐，规则见根 `CLAUDE.md` 的「身份指纹与接管」，改这两个文件前先读
- `Supervisor` MUST NOT 自己 spawn/abort task：需要副作用就 push 一个 `SupervisionEffect`，由 `frameworks` 的 `TaskBoard` 执行。本层不认识 tokio，`TimerState` 只存业务状态、`JoinHandle` 全在外层——两边字段一一对应，加一种效果要同步改 `SupervisionEffect` 枚举与 `TaskBoard::apply` 的 match
- 新增 Port 方法后 MUST 给 `test_helpers/ports_test_helpers.rs` 的 fake 补实现**并配单测**：fake 计入覆盖率门禁，只加不测就是未覆盖行

## 停止与强杀

- `stop_all_apps` 返回 `Vec<StopOutcome>`（不是 `Result`），落盘失败只记 warn：半路 `?` 会跳过其后为每个 outcome 排 `schedule_force_kill` 的循环与 unswept 清扫 ⇒ 服务已 SIGTERM、内存已 `Stopping`，却没有任何强杀定时器，`kill_timeout_ms` 形同虚设
- `stop_all` 里 `terminate` 失败 MUST 仍 `mark_stopping`：记录留在 `Online` 时，unswept 清扫照样为该 pid 排延时强杀，随后的退出事件会走 `classify_exit` → `decide_restart` 被当成崩溃自动重启（症状：`pm3 stop-all` 几分钟后服务自己回来了）
- `on_force_kill` 的 generation 守卫 MUST 在**有 token 时**让路：`delete` 后同名 `start` 会 `bump` generation，把 delete 前排定的强杀判成 stale 丢弃 ⇒ 顽固旧进程与新实例双开且无补偿路径。有 token 时改由 `sweep_pid` 的 `pid_was_recycled` 守卫兜住（比 generation 更准）；无 token 才保留 generation 守卫（否则裸发信号会打到复用 pid）
- `delete` MUST NOT 清掉服务的 generation：`current_generation` 对未知名字返回哨兵 `0` 而 `is_current(name, 0)` 恒为真 ⇒ generation 守卫形同虚设；同时真实退出事件带着 generation≥1 抵达 `on_exit` 会因不匹配而提前 return，`CancelForceKill` 永不发出 ⇒ 延时强杀必定走完 `kill_timeout_ms` 并可能打到复用 pid。generation 是全局单调计数，同名服务重建不会撞号，本就不需要清；`on_exit` 里改用「表里已无此记录」判定并记 debug
- `schedule_restart` 的 `JoinHandle` 存进 `TimerBoard.restarts`，`stop`/`delete`/`stop_all` 三条路径都 `cancel_restart`，`on_restart` 先 `claim_restart` 再执行（抢在 abort 之前入队的事件因此被丢弃）：只 spawn 一个裸 sleep task 会让被停掉的服务自行复活，且每次崩溃多留一个孤儿 task

## 就绪探针

语义与终态规则在根 `CLAUDE.md`「就绪探针」，这里是本层的三处实现约束：

- **落盘状态是 `Launching` 且声明了探针的记录 MUST 保持 `Launching` 并重挂探针**（`adopt` 里判 `status != Launching || ready_probe.is_none()` 才 `mark_online`）：无条件 `mark_online` 会让「探针窗口内换代」的服务显示 online 而实际从未就绪，且 `await_ready_if_probing` 因状态已非 `Launching` 而跳过 ⇒ 探针永不执行、超时永不触发
- 依赖就绪后拉起 waiter（`launch_waiter`）**成功路径也 MUST `save_table`**：只在失败路径落盘会留下「进程在跑但 dump 里是 settled」的空窗，此刻 daemon 被 SIGKILL 就是永久孤儿 + 下次 start 双开
- 多个 `depends_on` MUST 全部登记：`DeferredStart.waiting_on` 是 `Vec<String>`（`waiting_dependencies` 收集所有未就绪依赖），`launch_waiter` 前先 `still_waiting` 复查其余依赖。只登记第一个依赖会让 C（依赖 A、B）在 B 仍 Launching 时上线，B 探针失败也不级联
- `stop`/`delete`/`stop_all` MUST 走 `cancel_ready`（中止探针 task + 从 waiters 摘除 + 级联取消），否则依赖就绪后会把用户已停掉的 Deferred 服务拉起来
