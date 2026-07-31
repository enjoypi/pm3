# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## Linux 平台验证

整条链路只在 macOS 上跑通过，以下按依赖顺序：

- [ ] `frameworks/tests/sandbox_isolation.rs:63` 的 `/usr/bin/nc` 是 macOS 路径，Debian 在 `/bin/nc` → 改为按平台取
- [ ] 在 Linux 容器内跑通 `just cov`；`bwrap` 需要 user namespace 权限，容器要加 `--cap-add SYS_ADMIN` 或 `--security-opt seccomp=unconfined`
- [ ] 验证 `pm3 service install` 落盘的 systemd user unit 路径正确
- [ ] 验证 `loginctl enable-linger` 不带用户名参数是否成立
