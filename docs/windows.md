# Windows 支持矩阵

pm3 的 Windows 支持以「服务安装与换代链可用」为目标：daemon 与全部 CLI 命令可在 Windows 10 1803+ 运行，服务管理器形态为 **Task Scheduler 的当前用户 OnLogon 任务**（免管理员，语义对齐 launchd / systemd --user）。Unix 专属能力按下表降级或拒绝。

## 可用

| 能力 | 说明 |
|---|---|
| `pm3 service install/uninstall`（含 `--dry-run`/`--force`） | 渲染 Task 2.0 XML + `<label>-daemon.cmd` 包装脚本，落在 `~/.pm3/service/`，经 `schtasks /Create /TN <label> /XML <file> /F` 注册并立即 `/Run` |
| `pm3 service`（状态） | `schtasks /Query /TN <label> /V /FO LIST`，输出含 `Running` 即视为运行中 |
| `pm3 install`（备份换代） | 与 Unix 同一链路；Task Scheduler 无 MainPID 概念，接管判定以 `pm3.pid` 为管理器 pid 的替代 |
| daemon / CLI 通信 | 命名管道 `\\.\pipe\pm3-<hash>`（hash 取自 socket 路径 + `<pm3.home>/pipe.secret`，其他用户读不到 secret 故无法预测管道名）；`pm3.sock` 仍存在但只是存在性标记文件，供「daemon 是否退净」判定 |
| `pm3 start/stop/restart/delete/list/logs/kill` | 停止与强杀经 `taskkill /PID <pid> /T /F`（/T 杀进程树，等价进程组语义） |
| 崩溃自愈 | 包装脚本恒以 `exit /b 1` 收尾，Task Scheduler 的 `RestartOnFailure` 必然触发 |

## 降级（行为与 Unix 不同）

| 能力 | 差异 |
|---|---|
| `pm3.service.restart_condition` | `on-failure` 降级为 `always`：包装脚本无法向 Task Scheduler 传递退出语义 |
| `pm3.service.restart_delay_secs` | Task Scheduler 的 RestartOnFailure 最小间隔为 1 分钟，更小的值被提升到 60 秒 |
| `pm3.service.{max_tasks,cpu_quota_percent,umask,wait_for_network}` | Task Scheduler 无对应物，渲染时忽略 |
| TERM / KILL | Windows 无信号语义，`taskkill /F` 一视同仁；优雅停机依赖 daemon 自身的控制台事件（CTRL_SHUTDOWN 落盘退出） |
| 文件权限 | 无 0600/0700 chmod（NTFS 用户目录天然 per-user）；`.env` 权限收紧、socket 属主校验均跳过 |
| 进程身份令牌 / 存活探测 | `/bin/ps` 不存在时全部走 `Unreadable` 路径：daemon 换代后服务一律 respawn、内存熔断不工作、`pm3 list` 的 CPU/RSS 列为空 |
| 日志切割探测 | 无 inode，`rename` 式轮转不检测；copytruncate（截断）仍被识别 |

## 不支持（fail-fast）

| 能力 | 行为 |
|---|---|
| 沙箱（`sandbox.mode` 为 `read-only` / `workspace-write`） | 无 seatbelt/bwrap 对应物，启动报 `no sandbox backend`。Windows 上必须把 `pm3.sandbox.mode` 设为 `danger-full-access` |
| peer 凭据校验 | 命名管道无 `SO_PEERCRED` 等价物；管道名混入 `<pm3.home>/pipe.secret`（NTFS 用户目录内，0600 语义）使其他用户既无法预测管道名也无法抢注 |
| `schtasks /Query` 的非英文输出 | 状态解析假定英文 locale；非英文系统上 `pm3 service` 恒报 `installed, not running` |

## 配置

- `pm3.service.schtasks_path`（默认 `schtasks`，走 PATH）——管理器二进制路径，与其他平台的管理器路径同一约定
- 环境变量注入：Task Scheduler XML 不支持环境变量，`HOME`/`PATH`/`PM3_*` 由包装脚本 `<label>-daemon.cmd` 逐行 `set`；值中的 `%` 会转义为 `%%`
