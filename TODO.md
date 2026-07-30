# TODO

pm3：极简版 pm2（带严格沙盒隔离）。计划全文见 `~/.claude/plans/jiggly-gathering-hejlsberg.md`。

## 当前状态

**四层全部完成，门禁全绿**：`just fmt` / `just lint` / `just cov`（675 测试，regions/functions/lines 四指标 100%，lcov 无 `,0` 项，无生产文件缺失）/ `just typecheck` / `just test-scripts` 均通过。

端到端验收（`frameworks/tests/`，各自独立 `PM3_HOME` tempdir、收尾 SIGTERM + 等退出）覆盖：全生命周期 CLI 链路、沙盒真隔离（cwd 内可写／cwd 外被拒／网络被拒）、崩溃熔断、依赖启动序与环检测、自动持久化与 resurrect、孤儿 socket 自愈、SIGINT 吞掉且 SIGTERM 退出。

## 待办

- [ ] 提交并推送（需用户明确同意）
- [ ] 在 Linux 容器内跑一遍 `just cov`：`bwrap` 需 user namespace 权限（`--cap-add SYS_ADMIN` 或 `--security-opt seccomp=unconfined`），且 `sandbox_isolation` 里 `nc` 的路径在 Debian 是 `/bin/nc`，需要按平台调整
- [ ] `README` 尚未写（新建文档需用户同意）
