# TODO

pm3：极简版 pm2（带严格沙盒隔离）。计划全文见 `~/.claude/plans/jiggly-gathering-hejlsberg.md`。

## 当前状态

**四层全部完成，门禁全绿**：`just fmt` / `just lint` / `just cov`（1157 测试，regions/functions/lines 四指标 100%，lcov 无 `,0` 项，无生产文件缺失）/ `just typecheck` / `just test-scripts` 均通过。

端到端验收（`frameworks/tests/`，各自独立 `PM3_HOME` tempdir、收尾 SIGTERM + 等退出）覆盖：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写／cwd 外被拒／网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 resurrect、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。

`pm3 service [install|uninstall] [--dry-run]`（不带子命令查状态）把 daemon 注册为用户级自启服务：macOS launchd LaunchAgent、Linux systemd user unit + `loginctl enable-linger`。生成的 plist 已过 `plutil -lint`，并在 macOS 真机跑通 install → running → `launchctl list` 核对 → uninstall。

启动参数与运行状态已彻底分离：`pm3 start --name X [选项] <程序> <参数...>` 把意图写成 `pm3.cfg_dir/<name>.yaml`（零绝对路径，`${HOME}` 占位、`script` 存裸名、`cwd` 由 daemon 推导），`dump.yaml` 只留 `services[].runtime`；daemon 启动时由 `SpecSource` 把两者缝起来，服务文件缺失/损坏只跳过并 `warn`。真机已验证 launchd 重启后 mihomo-rule 自动复活且代理连通。

daemon 自身重启不再连带重启服务：spawn 时把「身份令牌（`ps -o lstart=`）+ 启动参数摘要 + 二进制 sha256」记进 `dump.yaml`，重启后逐服务比对，全同则接管（`adopt`）已存活的进程并轮询监控，任一不同则先停掉旧幸存进程（`evict`）再重启。SIGTERM 只落盘退出、不动服务；彻底停机用 `pm3 kill --with-services`。子进程用独立 process group，launchd `AbandonProcessGroup` / systemd `KillMode=process`，bwrap 去掉 `--die-with-parent`。

cron 定时任务（架构照抄 pm2 `lib/Worker.js`：到点只调 `restart_app`，不引入新状态）：服务配置写 `schedule: "<5 字段 cron>"`，`autorestart:false` 时是一次性任务、`true` 时是定时重启常驻服务。随机语法用 OpenBSD 风格 `~`（`~`、`a~b`、`a~b/n`），由 `adapters/src/schedule/random_expand.rs` 在每次装定时器时展开成具体数字再交 croner，因此每次触发都重新摇。`pm3 list` 新增 `next` 列（本地 `HH:MM`，空表示无调度/已停），`describe` 显示 `schedule` 与带时区的 `next fire`。真机已跑通 `cargo-sweep` 每小时随机清理 `~/prj`、`~/contrib`、`~/sre`。

服务文件单体化：`cfg_dir/<name>.yaml` 不再包 `apps:` 数组（顶层直接是 `name:`/`script:`/…），`SpecSource::resolve_service` 用专属 `parse_service_file` 解析并按文件名核对 `name`；daemon↔CLI 的 start 请求改传服务名列表（`services: Vec<String>`）而非 apps 文件路径，服务文件仍是唯一事实来源。多服务 `apps:` 数组只保留在用户手写的 apps 文件（`pm3 start apps.yaml`）。

## 待办

- [ ] 在 Linux 容器内跑一遍 `just cov`：`bwrap` 需 user namespace 权限（`--cap-add SYS_ADMIN` 或 `--security-opt seccomp=unconfined`），且 `sandbox_isolation` 里 `nc` 的路径在 Debian 是 `/bin/nc`，需要按平台调整；顺带验证 `pm3 service install` 的 systemd 路径与 `loginctl enable-linger` 无用户名参数是否成立
- [ ] `README` 尚未写（新建文档需用户同意）
