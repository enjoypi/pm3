# usecases — 应用级业务规则

Interactor + Output Port（trait）。与外层交互只经 `ports/` 下的 trait，实现在 `adapters`、注入在 `frameworks`。

## 文件地图

| 文件 | 内容 |
|---|---|
| `start.rs` | `start_apps` / `start_one`；`StartMode::{Register, Execute}` |
| `stop.rs` / `restart.rs` / `delete.rs` | 对应 CLI 动作 |
| `resurrect.rs` | daemon 重启后逐服务比对指纹：adopt / evict / respawn |
| `supervise.rs` | 子进程退出与熔断监督循环 |
| `fingerprint.rs` | 身份指纹拼装 |
| `record.rs` / `persist.rs` | 运行态记录与落盘编排 |
| `query.rs` / `table.rs` / `selector.rs` | 查询、列表数据、`AppSelector` 解析 |
| `log_paths.rs` | 日志路径推导 |
| `ports/` | `clock` `dump_store` `fingerprint` `launcher` `probe` `scheduler` `signaler` `wrapper` |

## 本层规则

- 「注册时是否 spawn」的判定 MUST 落在 `start_apps` 的 `StartMode::Register` 分支，**不能**落在执行路径——cron 到点走的是执行路径，判在那里会让定时任务永不运行
- 新增 Port trait 时不要写 blanket impl：只有 trait 声明的文件进不了 lcov，会触发覆盖率门禁的「生产文件缺失」→ 让实现方显式 `impl Trait for X {}`
- `start_one` / `resurrect` 涉及身份指纹的采集时机与旧进程驱逐，规则见根 `CLAUDE.md` 的「身份指纹与接管」，改这两个文件前先读
