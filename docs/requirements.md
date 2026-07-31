# pm3 需求描述

pm3：极简版 pm2（带严格沙盒隔离）。单二进制，CLI 与常驻 daemon 合一，经 Unix socket 通信。

## 端到端验收

`frameworks/tests/` 下每个 e2e 用独立 `PM3_HOME` tempdir，覆盖：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写／cwd 外被拒／网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 resurrect、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。

## 自启服务

`pm3 service [install|uninstall] [--dry-run]`（不带子命令查状态）把 daemon 注册为用户级自启服务：macOS launchd LaunchAgent、Linux systemd user unit + `loginctl enable-linger`。

## 启动参数与运行状态分离

`pm3 start --name X [选项] <程序> <参数...>` 把意图写成 `pm3.cfg_dir/<name>.yaml`（零绝对路径，`${HOME}` 占位、`script` 存裸名、`cwd` 由 daemon 推导），`dump.yaml` 只留 `services[].runtime`；daemon 启动时由 `SpecSource` 把两者缝起来，服务文件缺失/损坏只跳过并 `warn`。

## 配置文件布局

`pm3.home`（默认 `~/.pm3`）放运行时状态：socket、pid 文件、日志、各服务工作目录，以及 daemon 自己的 `config.yaml`（`service install` 落盘那份，unit 的 `--config` 指向它）。`pm3.cfg_dir`（默认 `~/.config/pm3`）只放每服务一份 `<name>.yaml`。写 `~/.pm3/config.yaml` 与 `cfg_dir/<name>.yaml` 共用 `svc::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖；`service uninstall` 不删配置。`pm3.search_path` 是 PATH 的单一来源，既写进 launchd/systemd unit，也是 daemon 解析 app 程序名的搜索路径。

## daemon 重启与服务接管

spawn 时把「身份令牌（`ps -o lstart=`）+ 启动参数摘要 + 二进制 sha256」记进 `dump.yaml`，daemon 重启后逐服务比对：全同则接管（`adopt`）已存活的进程并轮询监控，任一不同则先停掉旧幸存进程（`evict`）再重启。SIGTERM 只落盘退出、不动服务；彻底停机用 `pm3 kill --with-services`。子进程用独立 process group，launchd `AbandonProcessGroup` / systemd `KillMode=process`，bwrap 去掉 `--die-with-parent`。

## cron 定时任务

架构照抄 pm2 `lib/Worker.js`：到点只调 `restart_app`，不引入新状态。服务配置写 `schedule: "<5 字段 cron>"`：`autorestart: false` 时是一次性任务，`true` 时是定时重启常驻服务。随机语法用 OpenBSD 风格 `~`（`~`、`a~b`、`a~b/n`），由 `adapters/src/schedule/random_expand.rs` 在每次装定时器时展开成具体数字再交 croner，每次触发都重新摇。`pm3 list` 的 `next` 列显示本地 `HH:MM`（空表示无调度/已停），`describe` 显示 `schedule` 与带时区的 `next fire`。

## 服务文件格式

`cfg_dir/<name>.yaml` 是单体格式（顶层直接 `name:`/`script:`/…，不包 `apps:` 数组），`SpecSource::resolve_service` 用专属 `parse_service_file` 解析并按文件名核对 `name`；daemon↔CLI 的 start 请求传服务名列表（`services: Vec<String>`），服务文件是唯一事实来源。多服务 `apps:` 数组只保留在用户手写的 apps 文件（`pm3 start apps.yaml`）。
