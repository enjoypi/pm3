# Windows 能力矩阵

pm3 的 daemon 与全部 CLI 命令可在 Windows 10 1803+ 运行，自启形态是 **Task Scheduler 的当前用户登录触发任务**（免管理员，语义对齐 macOS / Linux 的用户级自启）。Unix 专属能力按下表降级或拒绝。

## 可用

| 能力 | 说明 |
|---|---|
| `pm3 service install/uninstall`（含 `--dry-run`/`--force`） | 自启配置落在 `~/.pm3/service/`，注册后立即启动 |
| `pm3 service`（状态） | 查询已注册任务的运行状态 |
| `pm3 install`（备份换代） | 与 Unix 同一链路；接管判定以 pid 文件为准（Task Scheduler 无管理器 pid 概念） |
| daemon / CLI 通信 | 命名管道；管道名混入 `~/.pm3` 下一份只有属主可读的随机 secret，其他用户既无法预测也无法抢注 |
| `pm3 start/stop/restart/delete/list/logs/kill` | 停止与强杀连带整棵进程树，等价 Unix 的进程组语义 |
| 崩溃自愈 | 任务失败即重启，效果等同「总是重启」 |

## 降级（行为与 Unix 不同）

| 能力 | 差异 |
|---|---|
| `pm3.service.restart_condition` | `on-failure` 降级为 `always`：无法向 Task Scheduler 传递退出语义 |
| `pm3.service.restart_delay_secs` | Task Scheduler 重启最小间隔为 1 分钟，更小的值被提升到 60 秒 |
| `pm3.service.max_tasks` / `cpu_quota_percent` / `wait_for_network` | Task Scheduler 无对应物，渲染时忽略 |
| TERM / KILL | Windows 无信号语义，强杀一视同仁；优雅停机依赖 daemon 自身的控制台关机事件（落盘后退出） |
| 文件权限 | 无 0600/0700 chmod（NTFS 用户目录天然 per-user）；`.env` 权限收紧、socket 属主校验均跳过 |
| 进程身份令牌 / 存活探测 | 无 `/bin/ps` 时全部走「读不出」路径：daemon 换代后服务一律重启、内存熔断不工作、`pm3 list` 的 CPU/RSS 列为空 |
| 日志切割探测 | 无 inode，rename 式轮转不检测；copytruncate（截断）仍被识别 |

## 不支持（fail-fast）

| 能力 | 行为 |
|---|---|
| 沙箱（`sandbox.mode` 为 `read-only` / `workspace-write`） | 无 seatbelt/bwrap 对应物，启动报 `no sandbox backend`。Windows 上必须把 `pm3.sandbox.mode` 设为 `danger-full-access` |
| peer 凭据校验 | 命名管道无 `SO_PEERCRED` 等价物，改由不可预测的管道名兜住（见上表） |
| 非英文 locale 的自启状态查询 | 状态解析假定英文输出；非英文系统上 `pm3 service` 恒报 `installed, not running` |

## Windows 专属配置

- `pm3.service.schtasks_path`（默认 `schtasks`，走 PATH）——自启管理器路径，与其他平台的管理器路径同一约定
- `pm3.service.taskkill_path`（默认 `taskkill`，走 PATH）——停止与强杀用的进程工具路径；unix 侧是硬约束的 `/bin/kill`，故无对应键
