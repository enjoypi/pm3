# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## macOS 上的覆盖率门禁

`just cov` 在 macOS 上挂三处（`4064e42` 基线即如此，与后续改动无关，Linux 上应为 100%）：

- [ ] `frameworks/src/layout.rs` 的 `host_uid()`：`/proc/self` 在 macOS 不存在 ⇒ `.map(|owner| owner.uid())` 闭包永不执行
- [ ] `adapters/src/process/ps_probe.rs:122-124`：`wait_for_exit` 里「预算未耗尽 ⇒ sleep 后再探一轮」的路径未被走到
- [ ] `adapters/src/process/watcher.rs:128,165,172,175`：轮询后仍有等待者、`Liveness::Unreadable`、pid 复用三条分支未被走到
