# usecases — 应用级业务规则

Interactor + Output Port（trait）。与外层交互只经 `ports/` 下的 trait，实现在 `adapters`、注入在 `frameworks`。

## 文件地图

| 文件 | 内容 |
|---|---|
| `supervisor.rs` | `Supervisor`：daemon 全部编排的落点，吃 `SupervisionRequest`/生命周期事件，吐 `(结果, Vec<SupervisionEffect>)` |
| `supervision.rs` | 边界契约：`SupervisionRequest` / `SupervisionReply` / `SupervisionFailure` |
| `supervisor_log.rs` | `Supervisor` 的业务日志（`log_*`），MUST 是 `pub`——私有模块内的 `pub(crate)` 触发 clippy `redundant_pub_crate` |
| `timer_state.rs` | `TimerState`：定时器/待重启/generation 的**业务状态**，不持 `JoinHandle` |
| `start.rs` | `start_apps` / `start_one`；`StartMode::{Register, Execute}` |
| `stop.rs` / `restart.rs` / `delete.rs` | 对应 CLI 动作；`persist_for_handover` 是 daemon 换代收尾（只落盘，不改状态） |
| `resurrect.rs` | daemon 重启后逐服务比对指纹：adopt / evict / respawn |
| `supervise.rs` | 子进程退出与熔断监督循环 |
| `fingerprint.rs` | 身份指纹拼装 |
| `record.rs` / `persist.rs` | 运行态记录与落盘编排 |
| `query.rs` / `table.rs` / `selector.rs` | 查询、列表数据、`AppSelector` 解析 |
| `log_paths.rs` | 日志路径推导 |
| `ports/` | `clock` `dump_store` `fingerprint` `launcher` `probe` `scheduler` `signaler` `wrapper` |

## 本层规则

- 「注册时是否 spawn」的判定 MUST 落在 `start_apps` 的 `StartMode::Register` 分支，**不能**落在执行路径——cron 到点走的是执行路径，判在那里会让定时任务永不运行
- 批处理 Interactor MUST NOT 在循环里 `?`：`start_apps` 返回 `StartReport { outcomes, failure }`（不是 `Result`），`resurrect` 逐服务记 warn 后继续。半路 `?` 会把「已 spawn / 已 adopt」的 outcome 一起丢掉，调用方的 `watch_all` 与末尾的 `save_table` 都不执行 → 进程在跑却没有 watch task，autorestart 与熔断全失效，dump 里也没有它的 pid，daemon 一重启就成永久孤儿
- `resurrect` 的 `topo_sort` 失败 MUST 降级为「按表序恢复」而非中止：`delete` 留下的悬空 `depends_on` 或缺失的服务文件都会让整图排不出序，一 `?` 就把全部幸存进程弃管。`stop_all_apps` 同理（原先的 `unwrap_or_default()` 更糟：一个都不停却报成功）
- 新增 Port trait 时不要写 blanket impl：只有 trait 声明的文件进不了 lcov，会触发覆盖率门禁的「生产文件缺失」→ 让实现方显式 `impl Trait for X {}`
- `start_one` / `resurrect` 涉及身份指纹的采集时机与旧进程驱逐，规则见根 `CLAUDE.md` 的「身份指纹与接管」，改这两个文件前先读
- `Supervisor` MUST NOT 自己 spawn/abort task：需要副作用就 push 一个 `SupervisionEffect`，由 `frameworks` 的 `TaskBoard` 执行。本层不认识 tokio，`TimerState` 只存业务状态、`JoinHandle` 全在外层——两边字段一一对应，加一种效果要同步改 `SupervisionEffect` 枚举与 `TaskBoard::apply` 的 match
- 新增 Port 方法后 MUST 给 `test_helpers/ports_test_helpers.rs` 的 fake 补实现**并配单测**：fake 计入覆盖率门禁，只加不测就是未覆盖行
