# TODO

pm3：极简版 pm2（带严格沙盒隔离）。需求描述见 `docs/requirements.md`。

## 待办

- [ ] 在 Linux 容器内跑一遍 `just cov`：`bwrap` 需 user namespace 权限（`--cap-add SYS_ADMIN` 或 `--security-opt seccomp=unconfined`），且 `sandbox_isolation` 里 `nc` 的路径在 Debian 是 `/bin/nc`，需要按平台调整；顺带验证 `pm3 service install` 的 systemd 路径与 `loginctl enable-linger` 无用户名参数是否成立
- [ ] `README` 尚未写（新建文档需用户同意）
