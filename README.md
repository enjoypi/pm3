# pm3

极简版 pm2，带严格沙盒隔离的进程管理器：单 Rust 二进制，CLI 与常驻 daemon 合一，经本机 Unix socket 通信（Windows 用命名管道），不占网络端口。面向「自己机器或一台服务器上托管几个常驻程序与定时任务」的场景，被托管的程序默认拒网、默认只能写自己的工作目录。

设计动机与完整需求见 [docs/requirements.md](docs/requirements.md)；Windows 能力矩阵见 [docs/windows.md](docs/windows.md)。

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

**Windows**（PowerShell，不使用 .sh；产物是 `pm3-<版本>-x86_64-pc-windows-msvc.zip`，内含 `pm3.exe`、`LICENSE`、`config.yaml`）：

```powershell
$tag = (Invoke-RestMethod https://api.github.com/repos/enjoypi/pm3/releases/latest).tag_name
$zip = "pm3-$tag-x86_64-pc-windows-msvc.zip"
Invoke-WebRequest "https://github.com/enjoypi/pm3/releases/download/$tag/$zip" -OutFile $zip
Invoke-WebRequest "https://github.com/enjoypi/pm3/releases/download/$tag/$zip.sha256" -OutFile "$zip.sha256"
$expected = (Get-Content "$zip.sha256" -Raw).Split(' ')[0]
if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() -ne $expected) { throw "sha256 校验失败" }
Expand-Archive $zip -DestinationPath pm3
.\pm3\pm3.exe --config .\pm3\config.yaml install   # 首次安装
pm3 install                                        # 之后的升级
```

**从源码装**（任何平台，需要 Rust 工具链；Windows 用 msvc 工具链）：

```sh
cargo install --git https://github.com/enjoypi/pm3 --bin pm3 --locked
```

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

`start` 的开关：

| 开关 | 作用 |
|---|---|
| `--cwd <dir>` | 工作目录，默认 `<pm3 home>/<name>` |
| `--network` | 放行出站网络 |
| `--writable-dir` / `--readable-dir` | 在沙箱上开口子，可重复 |
| `--max-memory 300M` | 常驻内存超限自动重启 |
| `--cron '0 ~ * * *'` | 定时重启，支持 OpenBSD 的 `~` 随机语法 |
| `--ready-exec <cmd>` / `--ready-tcp <host:port>` | 就绪探针（二者互斥），通过才转 online；`--ready-exec` 可重复，第一个是程序名 |
| `--listen-timeout <ms>` | 就绪总预算，超时即判出错 |
| `--stop-exit-code <n>` | 该退出码视为正常停止，不触发重启，可重复 |
| `--no-autorestart` | 退出后不自动拉起（配 `--cron` 即一次性定时任务） |
| `--force` | 覆盖已存在的服务文件 |

完整说明见 `pm3 start --help`；shell 补全用 `pm3 completion <bash|zsh|fish>`。

**默认沙箱行为**：每个服务只能写自己的工作目录（`workspace-write`），读面是系统目录白名单加程序自身（`read: minimal`），默认拒网。Linux 后端是 bwrap，macOS 是 seatbelt。pm3 自身的两个目录恒从沙箱里挖掉，服务永远摸不到 socket 与凭据。要完全放开就在该服务的 yaml 里写 `mode: danger-full-access`（自带沙箱的程序如 sshd 必须用它，因为沙箱不能嵌套）。

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
| daemon 常驻内存 | 个位数 MiB | 百 MiB 级 |
| 凭据 | 每服务 `.env` 文件，0600 | 配置文件内嵌 |
| cluster 负载均衡 | 无 | 有（Node 进程内） |
| 生态与文档 | 新项目，小而锐 | 成熟，社区庞大 |

要管 Node 大集群、要 cluster 模式的进程内负载均衡，选 pm2；要在自己机器上低开销地管几个常驻服务、且希望它们默认被关进沙箱，选 pm3。本机实测常驻内存与启停延迟可跑 `just bench`。

## 与 docker/podman 相比

两者不互斥：容器给的是**镜像分发与强隔离**（依赖打包、独立网络栈、可复现部署），pm3 给的是**裸进程的轻量托管**（秒级启动、直接读写宿主文件、没有镜像构建这一步）。pm3 的沙箱是「降权」不是容器——服务与宿主共享内核，隔离强度弱于容器，但开销也低到可以忽略。

要分发给别人的机器、要可复现环境、要强隔离，用容器；要在自己机器上管几个长期跑的小服务与定时任务，用 pm3。

## 许可

MIT，见 [LICENSE](LICENSE)。
