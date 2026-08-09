# pm3

极简版 pm2，带严格沙盒隔离的进程管理器：单 Rust 二进制，CLI 与常驻 daemon 合一，经本机 Unix socket 通信（Windows 用命名管道），不占网络端口。面向「自己机器或一台服务器上托管几个常驻程序与定时任务」的场景。

**为什么新做一个**：pm2 没有沙箱，被托管的进程默认能读满盘、连满网；它自身背着 Node 运行时与上百兆常驻内存。pm3 要的是反过来的默认：**裸进程 + 默认拒网、默认只能写自己的工作目录**，管理器自身只是一个几 MiB 的二进制。

**设计理念**：

1. 单二进制、零运行时依赖（除了 `/bin/ps` 与 `/bin/kill`），装完即用
2. 沙箱默认开：每个服务只能写自己的 cwd、拒网、只读白名单，开口子要显式声明
3. 配置即文件：每个服务一份 yaml、凭据一份 `.env`，意图与运行状态分目录存放，没有数据库

设计动机与完整需求论证见 [docs/requirements.md](docs/requirements.md)；Windows 支持矩阵见 [docs/windows.md](docs/windows.md)。

## 安装

**一行装**（macOS / Linux；校验 sha256、落 `~/bin/pm3`、自动跑 `pm3 install` 注册开机自启）：

```sh
curl -fsSL https://raw.githubusercontent.com/enjoypi/pm3/main/install.sh | sh
```

**手动下载**（macOS / Linux）：从 [GitHub Releases](https://github.com/enjoypi/pm3/releases/latest) 下对应平台的 tar.gz 与同名 `.sha256`，校验后解开（内含 `pm3`、`LICENSE`、`config.yaml`）：

```sh
shasum -a 256 -c pm3-<版本>-<平台>.tar.gz.sha256   # Linux 用 sha256sum -c
tar xzf pm3-<版本>-<平台>.tar.gz
./pm3 --config ./config.yaml install              # 首次安装
pm3 install                                       # 之后的升级
```

macOS：二进制未签名，浏览器下载的首次运行前 `xattr -d com.apple.quarantine pm3`（curl 下载不带该属性，可跳过）。

**从源码装**（需要 Rust 工具链；**Windows 目前走这条路**，msvc 工具链）：

```sh
cargo install --git https://github.com/enjoypi/pm3 --bin pm3 --locked
```

Windows 的能力矩阵（Task Scheduler 自启、命名管道 IPC、沙箱降级项）见 [docs/windows.md](docs/windows.md)。

**运行时依赖**（macOS / Linux，三条都必须满足，否则功能静默退化；Windows 无这些依赖）：

- `/bin/ps` 与 `/bin/kill`（procps）：缺了每次 daemon 重启所有服务都会被判探测失败而驱逐重启
- Linux 沙箱需要 `bwrap`（bubblewrap）：缺了沙箱模式起不来
- macOS 用系统自带 seatbelt，无需额外安装

升级就是重跑安装命令：`pm3 install` 自带备份（`~/.pm3/install-backups/<旧版本>/`）、原子换二进制、重装自启、核对接管。

## 使用

```sh
pm3 start --name web --cwd ~/sites/web node server.js   # 托管一个程序
pm3 list                                                # 一屏看状态
pm3 logs -f web                                         # 跟随日志
pm3 restart web                                         # 重启（重读配置与 .env）
pm3 stop web && pm3 delete web                          # 停止并遗忘
pm3 install                                             # 注册 pm3 自身开机自启（launchd / systemd user / Task Scheduler）
```

`start` 的常用开关：`--network`（放行出站网络）、`--writable-dir` / `--readable-dir`（开口子）、`--max-memory 300M`（超限自动重启）、`--cron '0 ~ * * *'`（定时重启，支持 OpenBSD 的 `~` 随机语法）、`--ready-tcp 8080`（就绪探针）、`--no-autorestart`。完整列表见 `pm3 start --help`；shell 补全用 `pm3 completion <bash|zsh|fish>`。

**默认沙箱行为**：每个服务只能写自己的工作目录（`workspace-write`），读面是系统目录白名单加程序自身（`read: minimal`），默认拒网。Linux 后端是 bwrap，macOS 是 seatbelt。pm3 自身的两个目录恒从沙箱里挖掉，服务永远摸不到 socket 与凭据。要完全放开就写 `mode: danger-full-access`（自带沙箱的程序如 sshd 必须用它）。

**凭据约定**：服务的环境变量只来自 `~/.config/pm3/<name>.env`（`KEY=VALUE` 一行一个，pm3 自动收紧到 0600，只读不写）。轮换凭据 = 改文件 + `pm3 restart <name>`。

**两个目录**：

| 目录 | 放什么 |
|---|---|
| `~/.pm3`（运行时状态） | socket、pid、`dump.yaml`（运行状态落盘）、`pm3.log`、各服务日志与工作目录、换代备份 |
| `~/.config/pm3`（意图声明） | 每服务一份 `<name>.yaml` 配置 + 可选 `<name>.env` 凭据 |

全局配置在 `~/.pm3/config.yaml`，`pm3 config check` 校验、`pm3 config show` 看最终生效值。

## 与 pm2 相比

| | pm3 | pm2 |
|---|---|---|
| 运行时依赖 | 无（单 Rust 二进制） | Node.js + npm 包 |
| 沙箱 | 默认开（拒网、只写 cwd） | 无 |
| daemon 常驻内存 | ~8 MiB（见下节实测） | 百 MiB 级 |
| 凭据 | 每服务 `.env` 文件，0600 | 配置文件内嵌 |
| cluster 负载均衡 | 无 | 有（Node 进程内） |
| 生态与文档 | 新项目，小而锐 | 成熟，社区庞大 |

要管 Node 大集群、要 cluster 模式的进程内负载均衡，选 pm2；要在自己机器上低开销地管几个常驻服务、且希望它们默认被关进沙箱，选 pm3。

## 与 docker/podman 相比

两者不互斥，解决的问题不同：容器给的是**镜像分发与强隔离**（依赖打包、独立网络栈、可复现部署），pm3 给的是**裸进程的轻量托管**（秒级启动、直接读写宿主文件、没有镜像构建这一步）。pm3 的沙箱是「降权」不是容器——服务与宿主共享内核，隔离强度弱于容器，但开销也低到可以忽略。

边界：要分发给别人的机器、要可复现环境、要强隔离，用容器；要在自己机器上管几个长期跑的小服务与定时任务，pm3 合适。

## 性能数据

实测（Linux arm64 2 核，pm3 1.11.0，2026-08-09 18:52 UTC+8；`just bench` 可重跑，脚本在 `dev_scripts/bench.ts`）：

| 指标 | 数值 |
|---|---|
| 冷启动（含拉起 daemon） | 46 ms |
| 冷启动（接管 8 个在跑服务） | 1149 ms |
| pm3 list 热路径（n=50） | mean 9 ms / median 9 ms / p95 10 ms |
| start 到 Online（n=8） | mean 171 ms / median 170 ms / p95 174 ms |
| daemon 空载 RSS | 7.6 MiB |
| daemon 带 8 服务 RSS | 7.9 MiB（每服务开销 43 KiB） |
| 8 个被托管进程自身 RSS 合计 | 13.1 MiB |

## 许可

MIT，见 [LICENSE](LICENSE)。
