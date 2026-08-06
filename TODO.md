# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

- [ ] 在 macOS 上跑一次 `just lint` + `just cov --fresh` 复核：四处平台性缺口的修法（`layout.rs` 的 `owner_uid_of`、`ps_probe.rs` 的重探测试、`watcher.rs` 的自计数 fake ps、`private_file.rs` 的 `fill`）都是在 Linux 上验证的，两平台应同为 100%

## 安装能力搬进二进制

- [ ] `pm3` 自己要能完成全部安装动作，不依赖仓库里的 `dev_scripts/install.ts`：拿到单个二进制的用户现在装不出同样的效果（脚本独有的是备份、原子换二进制、`uninstall → kill → 等 daemon 退净 → install --force` 的换代顺序、等「服务管理器报的 pid == `pm3.pid`」、before/after 服务对比与 `lost` 判定）。目标形态是 `pm3 install` 自己走完，`just install` 退化成「opt-level 3 构建 + 调它」
  - 换代顺序是硬约束（根 `CLAUDE.md`「装真机与换代」），搬迁 MUST 逐条保住，尤其「install 后等 pid 对齐再跑任何 CLI 命令」——否则 `ensure_daemon_running` 会拉起非托管 daemon 抢 socket
  - **难点是「二进制自己换掉自己」**：脚本能先 `kill` 再 `cp`（`Text file busy` 因此绕开），而 `pm3 install` 是运行中的那个二进制去覆写自己。`.incoming` + `rename` 已是原子的，但发起者仍在运行 ⇒ 需要设计谁在最后一步落地（候选：新二进制自己 `service install`；或换完只做 rename、由服务管理器拉起新代）
  - 备份策略沿用 `backupRoot`（`<pm3.home>/install-backups/<时间戳>/`，目录 0700）

## 对照 pm2 还可以补的功能

来源 `docs/pm2-comparison.md`，按那里的优先级；P2 那批（cluster/scale/零停机 reload/watch/deploy/模块/programmatic API/APM）已判定与「极简+单机+强隔离」定位冲突，**不进本清单**。

- [ ] `pm3 logs --err` / `--all`：`-err.log` 一直在落盘，但 CLI 只暴露 `stdout_log`（`frameworks/src/commands.rs`），看错误日志只能自己 `cat`。复用 `usecases::log_paths::STDERR_SUFFIX`，走 `stdout_log` 同一条名字校验
- [ ] `--json` 输出（`list` / `describe`）：`adapters/src/presenter/` 加一个 JSON presenter 与 `table.rs`/`describe.rs` 并列，MUST 沿用「不含 env」的字段集。没有它，脚本与监控消费不了 pm3 的状态
- [ ] 就绪探针（`ready_probe`: exec/tcp + `listen_timeout`）：`DepGraph` 目前只保证 spawn 先后，不保证被依赖者真的可服务——这是依赖编排的配套语义缺口。判定放 `usecases`，探测实现放 `adapters`
- [ ] 指数退避重启：扩 `entities/src/process/restart.rs::decide_restart`（纯函数，测试成本低），对应 pm2 的 `exp_backoff_restart_delay`；现在 `restart_delay_ms` 是固定值
- [ ] `list` 显示资源占用：RSS 已经在采样（内存熔断用），但只进判定不进表格；CPU 完全没采集
- [ ] 日志写侧 rotate（size 触发）：读侧 `LogFollower::reopen_if_rotated` 已兼容外部 rotate，自己不切割也不清空（也没有 `pm2 flush` 等价物）
- [ ] 多服务聚合 tail：`pm3 logs` 一次只能盯一个服务
- [ ] 手动重置熔断计数（`pm2 reset` 等价物）：跑满 `min_uptime` 会自动清零，所以影响有限；errored 后立刻 restart 再快速崩溃会当场再次熔断
