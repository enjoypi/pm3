# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

- [ ] 在 macOS 上跑一次 `just lint` + `just cov --fresh` 复核：四处平台性缺口的修法（`layout.rs` 的 `owner_uid_of`、`ps_probe.rs` 的重探测试、`watcher.rs` 的自计数 fake ps、`private_file.rs` 的 `fill`）都是在 Linux 上验证的，两平台应同为 100%
