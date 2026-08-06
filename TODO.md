# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## macOS 上的覆盖率门禁

`just cov` 在 macOS 上挂（Linux 上为 100%）：

- [ ] `frameworks/src/layout.rs` 的 `host_uid()`：`/proc/self` 在 macOS 不存在 ⇒ `.map(|owner| owner.uid())` 闭包永不执行
- [ ] `adapters/src/process/ps_probe.rs` 的 `wait_for_exit`：「预算未耗尽 ⇒ sleep 后再探一轮」的路径未被走到
- [ ] `adapters/src/process/watcher.rs`：轮询后仍有等待者、`Liveness::Unreadable`、pid 复用三条分支未被走到
- [ ] `adapters/src/tests/private_file_tests.rs` 的写失败用例带 `#[cfg(target_os = "linux")]`（靠 `/dev/full` 触发 ENOSPC），macOS 上少一条断言——该分支已改成尾表达式，不产生 region，只是断言覆盖面少一点

## 安全后续项

- [ ] dump 记 boot 标识（Linux `/proc/stat` 的 `btime`，macOS `sysctl kern.boottime`）：跨机器重启后一律视 pid 失效，根治 `resurrect` 里「无 token 仍要对陌生 pid 发信号」——当前只把爆炸半径从进程组缩到单 pid
- [ ] UDS peer credential 校验（`tokio::net::UnixStream::peer_cred`，无需 unsafe）：现在授权 100% 依赖 socket 0600 + 目录 0700，而目录 chmod 失败只 warn
- [ ] `POST /apps` 的 body 上限（`DefaultBodyLimit`）：现在只剩 axum 默认的 2 MB，单 actor 循环下是一条队头阻塞路径
- [ ] bwrap 补 `--unshare-ipc` / `--unshare-uts` / `--unshare-cgroup`（`--new-session` 不能加，会破坏进程组信号，原因见 `CLAUDE.md`）
- [ ] fork bomb 与 CPU 失控没有防线：`RLIMIT_NPROC` / seccomp 需要 `libc` + `unsafe`，与 workspace 的 `unsafe_code = "deny"` 冲突，先定取舍再动手
