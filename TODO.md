# TODO

pm3：极简版 pm2（带严格沙盒隔离）。计划全文见 `~/.claude/plans/jiggly-gathering-hejlsberg.md`。

## 当前状态

**四层全部完成，门禁全绿**：`just fmt` / `just lint` / `just cov`（675 测试，regions/functions/lines 四指标 100%，lcov 无 `,0` 项，无生产文件缺失）/ `just typecheck` / `just test-scripts` 均通过。

端到端验收（`frameworks/tests/`，各自独立 `PM3_HOME` tempdir、收尾 SIGTERM + 等退出）覆盖：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写／cwd 外被拒／网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 resurrect、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。

`pm3 service [install|uninstall] [--dry-run]`（不带子命令查状态）把 daemon 注册为用户级自启服务：macOS launchd LaunchAgent、Linux systemd user unit + `loginctl enable-linger`。生成的 plist 已过 `plutil -lint`，并在 macOS 真机跑通 install → running → `launchctl list` 核对 → uninstall。

## 待办

- [ ] 在 Linux 容器内跑一遍 `just cov`：`bwrap` 需 user namespace 权限（`--cap-add SYS_ADMIN` 或 `--security-opt seccomp=unconfined`），且 `sandbox_isolation` 里 `nc` 的路径在 Debian 是 `/bin/nc`，需要按平台调整；顺带验证 `pm3 service install` 的 systemd 路径与 `loginctl enable-linger` 无用户名参数是否成立
- [ ] `README` 尚未写（新建文档需用户同意）
