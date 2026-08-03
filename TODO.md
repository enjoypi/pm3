# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## Linux 平台验证

整条链路只在 macOS 上跑通过，以下按依赖顺序：

- [ ] 验证 `pm3 service install` 落盘的 systemd user unit 路径正确
- [ ] 验证 `loginctl enable-linger` 不带用户名参数是否成立
