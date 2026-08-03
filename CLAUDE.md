@~/.claude/CLAUDE.md

@~/.claude/clean-arch.md

@~/.claude/rust-p0.md

@~/.claude/rust-p1.md

pm3：极简版 pm2（带严格沙盒隔离）。单二进制，CLI 与常驻 daemon 合一，经 Unix socket 通信。

需求描述见 `docs/requirements.md`，本文件只记踩过的坑。**跨层的坑记在本文件，单层实现细节下沉到各 crate 的 `CLAUDE.md`。**

## 项目速览

| crate | 职责 | 层内坑 |
|---|---|---|
| `entities` | 业务对象与状态机：`AppSpec`/`ProcessStatus`/`RestartPolicy`/`DepGraph`/`SandboxPolicy` | `entities/CLAUDE.md` |
| `usecases` | Interactor 与 Port trait：`supervisor`（daemon 编排总入口）/`start`/`stop`/`restart`/`delete`/`resurrect`/`supervise`/`query` | `usecases/CLAUDE.md` |
| `adapters` | 格式转换与外部实现：config/http/persistence/presenter/process/sandbox/schedule/service/unit | `adapters/CLAUDE.md` |
| `frameworks` | 组装与入口：`main.rs`/`cli.rs`/`daemon/`/`client/`/`service.rs`/`signal.rs` | `frameworks/CLAUDE.md` |
| `arch_tests` | 依赖方向强制 | `arch_tests/CLAUDE.md` |

`dev_scripts/*.ts`（Bun）驱动 `just` 的复杂 recipe：`cov.ts` + `coverage_gate.ts`（覆盖率门禁）、`install.ts` + `install_plan.ts`（真机安装）、`monitor.ts`、`rename.ts`、`cargo_invocation.ts`。

| recipe | 作用 |
|---|---|
| `just build` | 编译 workspace |
| `just fmt` | nightly rustfmt 格式化（只有 nightly 能重排 import） |
| `just lint` | clippy 四组 lint 全开，任何 warning 即失败 |
| `just test` | 裸 nextest，不含覆盖率门禁 |
| `just cov` | **日常验收**：四指标 + lcov 真值 plate + 生产文件完整性自检；`--fresh` 清 workspace 重算 |
| `just install` | 装到真机：opt-level 3 构建、备份、原子换二进制、重装 unit、核对 pid 接管 |
| `just monitor <kind>` | tail 服务日志；`crash` 匹配 panic 与致命信号，`business` 匹配 error 与 WARN/ERROR |
| `just typecheck` | TS 严格检查，禁 `any` / 非空断言 / `ts-ignore` |
| `just test-scripts` | dev_scripts 的 TS 单元测试 |
| `just rename <name>` | 模板改名 |

## 命令与工作流

- 优先用 @justfile，禁止随手 `cargo`；例外：`just` 的 recipe 都是 workspace 级，单 crate 迭代用 `cargo <cmd> -p <crate> --release --offline`
- 改过 `Cargo.toml` 后 `--locked` 会直接失败，改用 `--offline`
- `just typecheck` / `just test-scripts` 前先 `bun install --frozen-lockfile`
- dev_scripts TS：`Bun.env.X` 触发 TS4111 → 写 `Bun.env["X"]`；`Bun.spawn` 不收 `readonly string[]` → 传 `[...command]`
- Bash 命令里出现字面量 `.env` 会被 hook 整条拦掉（报 `禁止读写 .env 文件`），而本仓到处是 `spec.env` / `entry.env` / `<name>.env` → 查这些内容用 Read / Grep 工具，别用 `rg`/`grep`/`sed` 的 Bash 命令；真机 `cfg_dir` 下的凭据文件连 Read 也被权限设置拒掉 → 验收「`.env` 生效了没」只能看日志与服务状态，别把「读一眼那个文件」写进步骤
- 手工验证要另建 pm3 home：scratchpad 路径太长，unix socket 会撞 macOS `SUN_LEN`（>104 字节，报 `path must be shorter than SUN_LEN`）→ 用 `mktemp -d`
- 改 `adapters/` 的目录结构、重命名类型、或把常量下沉进 `config.yaml` 后 MUST 跑 `just test-scripts`：`dev_scripts/tests/rust_contract.test.ts` 靠**字符串路径**读 Rust 源码来守「TS 常量与 Rust 常量一致」，路径失效后 `sourceOf` 对缺失文件返回空串、只报「no longer declared the way this guard reads it」而不说文件已搬走（本轮发现 4 个守卫因 `service/`→`unit/` 早已静默失效，另一个因 `SYSTEMCTL_PROGRAM` 下沉进配置而失效 → 改成读 `config.yaml` 的 `:-` 默认值）

### 装真机与换代

- 固定走 `just install`，MUST NOT 手工搬二进制（换代顺序有硬约束，见下）
- 排查真机先从 `~/.pm3/config.yaml` 读 `cfg_dir`：它与 `pm3.home` 各自独立配置（本机 `home=~/.pm3`、`cfg_dir=~/.config/pm3`、二进制在 `~/bin/pm3`），按 `<home>/service` 猜必定扑空
- `pm3 service install` 用 `current_exe()` 渲染 unit → MUST NOT 在仓库目录执行（会把 plist 钉在 `target/release/pm3`，一次 `cargo clean` 就起不来）；先把二进制 `cp` 到最终位置，再用**那个**二进制执行 install
- **症状**：`launchctl list` 的 PID 列是 `-`，job 已载入但 launchd 未监管、KeepAlive 形同虚设
  **原因**：任何 pm3 CLI 命令都会经 `ensure_daemon_running` 自动拉起一个**非 launchd 托管**的 daemon；它扛不住 `launchctl unload`，且会抢赢 socket 竞争让 launchd 那份直接退出
  **修法**：换代顺序 MUST 是 `service uninstall` → `pm3 kill` → 等 `pgrep -f "<bin> daemon"` 归零 → `service install --force`；install 后 MUST 等「launchd 报的 pid == `pm3.pid` 内容」再跑任何 CLI 命令，否则又会拉起竞争者。已处于未监管态时先 `pm3 kill` 停掉自启实例，再 `launchctl kickstart gui/$(id -u)/<label>` 交回 launchd
- unit MUST 导出安装时的 `PM3_*` 环境（`UnitSpec.pm3_env`，两个渲染器都写，值要排序否则 `reconcile` 的逐字节比对每次都判 Stale）：install 拷进 `pm3.home` 的 `config.yaml` 是**未做变量替换**的原文，而 unit 只导出 `HOME`/`PATH` 时，`${PM3_HOME:-~/.pm3}` 在服务管理器起的 daemon 里退回默认值 ⇒ daemon 在 `~/.pm3` 建 socket/pid/dump，而 CLI（shell 里有 `PM3_HOME`）去连 `/srv/pm3/pm3.sock` ⇒ 连不上就经 `ensure_daemon_running` 再拉起一个**非托管** daemon，正是本节开头那个坑。注意反过来把「替换后的文本」落盘会让 `reconcile` 每次 install 都判 Conflict
- 换代前 `cp` 二进制会撞 `Text file busy`（旧 daemon 还在跑）→ 先 uninstall + `pm3 kill` 再拷；`pkill -f '<path> daemon'` 会匹配到发起它的 shell 自身命令行、把自己一起杀掉（症状：命令 exit 144），排查残留只用 `pgrep`；但 `pgrep -f <pat>` 同样会匹配到发起它的 shell，按可执行名找用 `pgrep -x`
- Linux 侧同一套顺序换 `systemctl --user`，但三件事只在 Linux 成立：
  - **只有 `systemctl --user` 依赖 `XDG_RUNTIME_DIR`**（`loginctl` 走 system bus，不受影响——排查时别把两者混为一谈）：非登录会话（agent/CI shell）里它为空 → 走 systemctl 的调用失败，报文视 systemd 版本而异（`Failed to connect to bus: No medium found`，或 `Failed to connect to user scope bus via local transport: $DBUS_SESSION_BUS_ADDRESS and $XDG_RUNTIME_DIR not defined`）。**`just install` 与 `pm3 service *` 不再需要手工 export**：`host_runtime_dir()` 缺该变量时按 `/run/user/<uid>` 推导（uid 取 `/proc/self` 的属主，macOS 无 `/proc` 故为 `None`，launchd 也不需要），经 `UnitProgramSet.runtime_dir` 只注给 `systemctl --user` 那几条命令（`UnitCommand.env`）；`install.ts` 查 `MainPID` 时同样自己注入。手工敲 systemctl 排查时仍要自己 export
  - `loginctl enable-linger` **不带用户名是合法的**（`enable-linger [USER...]`，省略即作用于调用者）：无授权时它只报 polkit `requires interactive authentication`、**不报缺参数** → 别以为是命令写错了去补用户名。它走 polkit 授权，polkit 被 mask 或无交互授权时必失败，故在 install plan 里是 `UnitStep::TryRun`（失败只 warn，输出末尾追加 `skipped: ...`），MUST NOT 改回 `Run`：unit 与 enable 都已生效，整体报 rv=1 会让运维以为没装上。**linger 已启用时这一步整个不进 plan**（`linger_state` 先查，`LingerState::Enabled` 即跳过）→ 常见情形下 `just install` 不碰 polkit、无需任何特殊权限。仍看到 `skipped:` 说明 linger 真没开，由有 sudo 的账号补一次（已实测成功），否则用户注销后 user manager 回收会连带停掉 daemon
  - 查 linger MUST 传 uid（`loginctl show-user <uid> -p Linger --value`）：**不带用户名时它输出空串且 rv=0**（`show-user` 省略参数指“当前会话的用户”，非登录会话没有会话）→ 按「非零退出才是查不到」判会把「已开 linger」误读成未开，白跑一次必失败的 `enable-linger`。故 `loginctl_show_linger` 在 uid 未知时返回 `None`，判定退回 `Unknown`

## 改动波及清单

改这里就必须同步那里，漏一处即编译失败或运行期对不上。

- 给 `Pm3Config` 加字段要同步 6 处：根 `config.yaml`、`adapters/test_support/config_sections.rs`、`adapters/src/test_helpers/config_schema_test_helpers.rs`、`frameworks/test_support/config_fixtures.rs`、`frameworks/tests/common/mod.rs`、校验函数与 `every_error_variant` 表
- 给 `UnitSpec` 加字段要同步 4 处：`adapters/src/unit/spec.rs` 的结构体、`launchd.rs` 与 `systemd.rs` 两个渲染器、`adapters/test_support/unit_specs.rs` 的 `spec_for`；两个渲染器漏一个不会编译失败，症状是只有那个平台的 unit 少字段而另一平台测试全绿 → 新字段 MUST 在两份渲染器测试里各断言一次
- 给 `UnitProgramSet` 加字段要同步 4 处：`adapters/src/unit/command.rs` 的结构体与 `from_config`、`adapters/test_support/unit_specs.rs` 的 `program_set`、`frameworks/src/service.rs` 的 `ServiceContext` 与 `open_service_session`、`frameworks/src/tests/service_tests.rs` 的两处字面量；宿主派生值（uid、`XDG_RUNTIME_DIR`）MUST 由 `frameworks/src/layout.rs` 读一次 env 后经 `ServiceContext` 注入，MUST NOT 在 `adapters` 里读 env
- 给 `SpecSource` 加字段要同步 4 处：`adapters/src/apps_file/source.rs` 的结构体、`frameworks/src/daemon/service.rs`（唯一生产构造点）、`adapters/test_support/spec_sources.rs`、`frameworks/src/test_helpers/daemon_actor_test_helpers.rs` 与 `frameworks/src/tests/daemon_ports_tests.rs`；宿主派生值（`host_home`）MUST 由 `frameworks/src/layout.rs` 读一次 env 后注入
- 改 `usecases/src/ports/*` 里 trait 方法的签名要同步 3 个实现：`adapters` 侧的真实现（如 `YamlDumpStore`）、`frameworks/src/daemon/ports.rs` 的 `DaemonPorts` 转发、`usecases/src/test_helpers/ports_test_helpers.rs` 的 `FakePorts`；`FakePorts` 新增的 `seed_*` 若没人调用会触发 `dead_code`，且它计入覆盖率门禁
- 给 `ExitOutcome` 加变体要同步 4 处：`usecases/src/ports/launcher.rs` 的 enum 与 `failed()`、`adapters/src/process/tokio_launcher.rs`（子进程）、`adapters/src/process/watcher.rs`（adopt 来的进程）、`frameworks/src/daemon/timers.rs` 的 `unwrap_or`
- 给 `ProcessRuntime` 加字段要同步 4 处：`adapters/src/persistence/dto.rs` 的 `RuntimeDto` + `decode_state` + `encode_state`（两处都穷举解构）、`adapters/test_support/process_records.rs`；跨版本可读的字段一律 `#[serde(default)]`
- 给 `SandboxPolicy` 加字段会波及 ~13 处字面量（四层的 test_helpers/test_support）→ 加完先 `cargo build --workspace` 靠 E0063 逐个补齐
- 写 `cfg_dir/<name>.yaml` 的两条路径（apps 文件与 `pm3 start --name`）MUST 共用**同一个** `adapters::fold_entry`：它把 `script`/`cwd`/`args`/`sandbox.writable_roots` 四处折回 `${HOME}`/`${PM3_SERVICE_CWD}` 并对 roots 去重。曾经 frameworks 与 adapters 各有一份副本，已分歧到「inline 去重、apps 不去重」，同一份声明编码出两种 yaml → `pm3 start <apps-file>` 被 `reconcile` 拒绝（症状：diff 只差一行重复的 root，或全是 `-"${HOME}/x"` / `+"/Users/me/x"`）。新增含路径的字段只改 `fold_entry` 一处
- 新增 `${...}` 占位符 MUST 在 `substitute_env_vars` 里登记为保留名（`SERVICE_CWD_NAME` 那个分支），否则加载 cfg 文件时因「变量未设置且无默认值」直接报 `EnvVarNotSet`；保留名不支持 `:-` 默认值

## 领域不变量

跨层生效的业务规则；单层实现细节见各 crate `CLAUDE.md`。

### 进程与信号

- 停止/强杀 MUST 先对进程组发信号（`/bin/kill -<SIG> -- -<pid>`）、失败再退回单 pid：spawn 时 `process_group(0)` 让子进程自成组，只杀单 pid 会漏掉它 fork 的孙进程；adopt 来的进程可能不是组长，故回退分支必须保留
- 传给 `/bin/kill` 的 pid MUST 先过 `is_signalable`（`pid >= 2 && i32::try_from(pid).is_ok()`）：procps 对超 i32 的值**静默截断**——`4294967295` → `kill(-1)` 杀光当前用户所有进程（user manager/tmux/全部用户级服务一起没），`-4294967295` → `kill(1)`；macOS 的 BSD kill 严格报错，故此坑只在 Linux 炸。上一条的「组信号失败即回退单 pid」会把第一步的失败直接放大成第二步的灾难
- `Stopping` 不是「已停止」：判「pm3 是否还持有进程」用 `ProcessStatus::is_settled()`（仅 Stopped|Errored），用 `!is_running()` 会让重复 `stop` 清空 pid、让 `restart` 再 spawn 一个同名实例
- SIGTERM 只落盘退出、不停服务，彻底停机只有 `pm3 kill --with-services`
- daemon 换代（shutdown）MUST NOT 把 `Stopping` 记录改写成 `Stopped`：`persist_for_handover` 只落盘、保留状态与 pid。改写会清掉 identity，让下一代 `resurrect` 的 `!is_settled()` 筛选整条跳过它 —— 既不 evict 也不监控，drain 未完的进程永久残留，随后一次 `start` 就起出第二份实例。对应地 `resurrect` 对 `Stopping` 记录走 `Verdict::Settle`：先 `evict` 掉幸存者再记 `Stopped`（把上一代没做完的 stop 做完），MUST NOT 让它走 `Adopt` —— 那会把运维明确停掉的服务重新拉回 `Online`
- 所有强杀入口 MUST 共用一条带守卫的路径（`Supervisor::sweep_pid`：查 `tracked_pids` → `pid_was_recycled` 比对 token → `force_kill`，失败记 warn）。曾经三个入口各实现一半：`on_force_kill` 三道守卫齐全、`resurrect::judge` 的 `Verdict::Settle` 一道没有、`Daemon::force_kill_survivors` 在 frameworks 层裸发 SIGKILL —— 后两条在 pid 复用后会对无关进程组发信号（Linux 上尤其致命）。新增清扫入口时 MUST 复用 `sweep_pid`，MUST NOT 在 `frameworks` 层自己调 `ports.force_kill`
- `delete` MUST NOT 清掉服务的 generation：`current_generation` 对未知名字返回哨兵 `0` 而 `is_current(name, 0)` 恒为真 ⇒ generation 守卫形同虚设；同时真实退出事件带着 generation≥1 抵达 `on_exit` 会因不匹配而提前 return，`CancelForceKill` 永不发出 ⇒ 延时强杀必定走完 `kill_timeout_ms` 并可能打到复用 pid。generation 是全局单调计数，同名服务重建不会撞号，本就不需要清；`on_exit` 里改用「表里已无此记录」判定并记 debug
- 「延迟重启」在途期间 MUST 可被 `stop`/`delete`/`stop_all` 取消：`schedule_restart` 持 `JoinHandle` 存进 `TimerBoard.restarts`，三条路径都 `cancel_restart`，`on_restart` 先 `claim_restart` 再执行（抢在 abort 之前入队的事件因此被丢弃）。只 spawn 一个裸 sleep task 会让 `restart_delay_ms` 窗口内被停掉的服务自行复活，且每次崩溃多留一个孤儿 task
- 子进程环境默认为空（`tokio_launcher` 有 `env_clear()`），所以 spawn 前必须已解析出绝对路径

### 身份指纹与接管

- 指纹三要素记进 `dump.yaml`：身份令牌（`ps -o lstart=`）+ 启动参数摘要 + 二进制 sha256；daemon 重启后逐服务比对，全同则 `adopt` 已存活进程并轮询监控，任一不同则先 `evict` 旧幸存进程再重启
- 指纹 MUST NOT 含任何宿主环境派生值：`SandboxPolicy` 分 `writable_roots`（运维声明，进指纹）与 `derived_roots`（pm3 从 cwd/logs_dir/`$TMPDIR` 推导，不进指纹），沙箱授予 `granted_roots()` 并集。**两者是相加关系**：声明 `writable_roots` MUST NOT 清空 `derived_roots`，否则 `--writable-dir /srv/data` 会把服务自己的 cwd 踢出沙箱，进程一写工作目录就 EACCES 并进重启熔断；要授予「什么都不可写」用 `mode: read-only`，不要用空列表；`render_identity(&AppSpec)` 渲染声明而非包装后的 argv。踩过的坑：launchd 起的 daemon 有 `TMPDIR`、shell 起的没有 → 每次换代都误判 respawn
- 指纹 MUST 在 `start_one` spawn 成功那一刻采集：shutdown 时算会把「磁盘上的新哈希」当成旧进程的，重启后误判未变更 → 接管到跑着旧二进制的进程
- 防 pid 复用的身份令牌固定用 `LC_ALL=C ps -ww -o lstart= -p <pid>`（管道下不截断、`LC_ALL=C` 消 locale 漂移）；MUST NOT 换 `etime`（时长需容差）或加 `command=`（`spawn()` 返回时可能尚未 exec，拿到的是旧 argv）
- 存活探测 MUST 是三态 `Liveness::{Alive(token), Gone, Unreadable}`，MUST NOT 退回 `Option<String>`：把「ps 超时/缺失/非零退出」和「进程真的没了」混成 `None`，会让 `watcher` 把仍在跑的进程当已退出而重启（原进程脱离 `live` 集合成孤儿），并让 `resurrect` 跳过 `evict` 直接 respawn。`ps -p <pid>` 退出码 1 才是 `Gone`，其余非零退出是 `Unreadable`。`Unreadable` 时 `watcher` 继续轮询、`resurrect` 走 `respawn(stale: Some(pid))` 先杀后起（fail-safe）
- 指纹的输入 MUST 是运维声明的原文，MUST NOT 是 `canonicalize` 的结果：`materialise_workspace` 把解析后的路径**追加**进 `derived_roots`（不进指纹），`writable_roots` 原样保留。否则声明的 root 在首次启动时还不存在（canonicalize 失败回退字面量）、之后被创建或换成符号链接，digest 就变了 ⇒ 一个配置分毫未改、正常运行的服务每次换代都被 evict 后重启。沙箱靠 `granted_roots()` 同时授予声明值与真实路径，两者都授予是安全的
- 「子进程退出」MUST 是三态 `ExitOutcome::{Code(i32), Signalled, Unobserved}`，MUST NOT 退回 `Option<i32>`（与 `Liveness` 同一类教训）：`None` 会把「被信号打死」（真失败）与「adopt 来的进程读不到退出码」（未知）混成一个值，让 `settled_status` 把干净退出的被接管服务一律记成 `Errored` —— 同一个服务不换代时显示 `stopped`、跨过一次换代就显示 `errored`，靠状态列告警的监控当场误报。判失败用 `failed()`，`Unobserved` 不算失败
- 运行期监控 MUST 把 dump 里的身份令牌传给 `wait_for_exit`：只判「pid 还在不在」会在 pid 复用后永远等下去，随后的 `stop` 会对复用 pid 发进程组信号误杀整组
- 运行镜像 MUST 装 `procps`（`/bin/ps`）：缺了它每次 daemon 重启所有服务都被判「探测失败」而驱逐重启
- `resurrect` 判定 respawn 且旧进程仍存活（token 已匹配）时 MUST 先 `terminate` 掉它，否则孤儿与新实例重复运行（症状：`just cov` 跑完残留 `pm3 __sleep`）

### cron 调度

- 到点只调 `restart_app`、**不新增状态**（架构照抄 pm2 `lib/Worker.js`）
- `Fire` 事件 MUST 先比对 `timers.get(name).fire_at_ms == Some(fire_at_ms)` 再执行，否则已过期的定时器会误触发；`stop`/`delete`/`stop_all` 三条路径 MUST 走 `disarm`（remove + `JoinHandle::abort`）——`Daemon.timers: HashMap<name, Timer>` 同时是 `next` 列数据源、调度激活标记与过期判别依据，这样 `next` 有值即「等触发」、空即「真停了」，避开 pm2 那句 `stopped but CRON RESTART is still UP` 的语义混淆
- `Timer` MUST 持 `JoinHandle` 并在重新 `arm_timer` 时 abort 旧 task：只存 `fire_at_ms` 会让每次 restart 多留一个睡到旧 deadline 的孤儿 task（日更 cron + 每分钟 restart ⇒ 24h 累积上千个）
- 「这个服务的调度是否激活」MUST 落盘（`ProcessRuntime::schedule_armed`）：只存在内存 `timers` 里的话，daemon 换代后 `arm_known_timers` 会把用户 `stop` 掉的 cron 服务重新武装、到点自行复活

### CLI ↔ daemon 协议

- 是 JSON envelope `ReplyDto { report, service, already_running, refused }`：新增命令走 `ask_report`（只要文案）或 `ask`（要结构化字段）；MUST NOT 靠 `.contains(渲染文本)` 反解业务状态
- `start` 是**部分提交**的批处理，所以回滚粒度 MUST 与提交粒度一致：daemon 在「起了一部分」时回 200 + `refused`（未起来的服务名），CLI 只 `undo.run_for(&refused)` 并以 `Error::PartialStart` 结束（非零退出）；一个都没起来才回非 200、CLI 全量回滚。把部分成功当成「什么都没发生」而全量删服务文件，会让已在跑的服务下次 daemon 启动时 `rejoin` 失败被丢弃 → 永久孤儿
- `start` 的「某个服务起不来」与「起来了但落不了盘」MUST 是两个独立字段（`refused` / `unsaved`）：`refused` 由「requested 减 outcomes」反推，天然表达不了「全都起来了但 `dump.yaml` 写失败」⇒ 回 200 + 空 `refused` ⇒ CLI 退出码 0，而 dump 里没有这些服务，下次 daemon 重启 `resurrect` 读不到记录、既不 evict 也不监控 ⇒ 永久孤儿，CI 按退出码判定完全无感。`unsaved` 非空时 CLI MUST 非零退出（`Error::UnsavedStart`）且 MUST NOT 回滚服务文件（服务在跑）。`Supervisor::start` 只在 `outcomes` 为空时才返回 `Err`
- `start` 请求只传服务名列表（`services: Vec<String>`）——服务文件是唯一事实来源，MUST NOT 把 spec 塞进请求体
- CLI MUST 在请求头带 `x-request-id`（`adapters::REQUEST_ID_HEADER`，值取 `<CLI pid>-<序号>`），daemon 的 `request_id_of` 优先读它、缺失或空才回退内部 `AtomicU64`。回退分支 MUST 保留：换代期间旧版 CLI 不发这个头。少了这层，一次 `pm3 start` 的客户端日志与 daemon 日志无法串起来（daemon 侧计数器每次重启从 1 开始，跨进程毫无意义）

### 环境变量与凭据

- 服务的环境变量**只**来自 `cfg_dir/<name>.env`；`<name>.yaml` 的 `env` 字段已删除，残留的 `env:` MUST 报 `AppsFileError::EnvInYaml` 而非静默忽略（`AppEntry` 没有 `deny_unknown_fields`，不显式拒绝就是无声吞掉）。拒绝点有两个：`check_declared_names`（apps 文件走 `split_apps_file`，不 resolve，必须在 parse 期拦）与 `resolve_entry`（服务文件 + 兜底），共用 `reject_env`
- **读取点只有 `SpecSource::resolve_service` 一处**：`SpecResolver::prepare`（CLI start）与 `YamlDumpStore::rejoin`（daemon 换代逐条读盘）都经过它。只在 `prepare` 里读会让换代后所有服务 env 变空 ⇒ env 进指纹 ⇒ 全部服务被判 `Change::Launch` 而 evict+respawn
- `.env` **缺失 MUST 是 `Ok(空)`**，只有「存在但解析失败」才 `Err`：`rejoin` 拿到 `Err` 就没有 spec、造不出 `ProcessRecord`，那条记录进不了表
- **进不了表的记录 MUST 被 evict，MUST NOT 静默丢弃**：`DumpStore::load` 返回 `DumpContents { records, stranded }`，`resolve_service` 失败的那条以 `StrandedProcess { name, pid, token }` 进 `stranded`，`resurrect` 开头先 `sweep_stranded`（复用 `surviving_pid` 的 token 守卫 + `evict_pid`）。只 return None 的话，手改 `.env` 打错一个字再换代，正在跑的进程就既不 evict 也不监控、pid 还从 dump 擦除 ⇒ 永久孤儿 + 下次 `start` 起出第二份实例
- **`HOME` 由 pm3 注入**（`SpecSource.host_home` → `with_host_home`，`.env` 声明了同名 key 就以声明为准）：子进程环境被 `env_clear()` 清空，不注入的话服务只能在 `script`/`args` 里写死绝对路径。宿主 `$HOME` 只在 `frameworks/src/layout.rs::host_home()` 读一次再注入，adapters 层 MUST NOT 自己读 env。它随 `spec.env` 进指纹，安全的前提是 unit 会导出安装时的 `HOME`（见「装真机与换代」），launchd/systemd/shell 三种上下文取值一致
- `.env` MUST NOT 过 `substitute_env_vars`：那个替换器遇到含 `"` / `\` / 控制字符的值直接报错，还会把随机密码里的 `${...}` 当占位符展开。`.env` 自己只认**一个**变量：`$HOME` / `${HOME}` 展开成注入的 `host_home`（`PATH=$HOME/.cargo/bin:...` 这类要它），其余 `$` 一律原样。三条边界 MUST 保住：`$HOMEBREW_PREFIX` 不能被吃掉（`continues_a_name` 判词尾）、单引号值整体不展开（密码里真写了 `$HOME` 的逃生口）、`host_home` 为 `None` 时不展开
- 收紧 `.env` 权限（`chmod 0600`）MUST 先用 `symlink_metadata` 排除软链：`set_permissions` 跟随软链，把 `<name>.env` 链到 `/etc/creds/x.env` 这类共享凭据时会改到**别人的**文件上（症状：另一个消费者突然 EACCES，而 pm3 这边只有一条 debug 日志）
- 凭据 MUST NOT 出现在任何错误文案里：`EnvFileError` 只带路径、行号、key 名。曾经 `--env` 的 `InvalidEnvPair` 回显整个实参、yaml 冲突的 `diff_lines` 把新旧凭据一起打进 stderr —— 后者随「yaml 不再有 env」自动消失
- pm3 **只读不写** `.env`（CLI 的 `--env` 已移除），所以 `ServiceUndo` / `reconcile` 都不必管它；只有 `forget` 要连带删除
- `render_identity` / `encode_state` / `ProcessView` 三处都是**全字段解构**，给 `AppSpec` 加字段会编译失败而不是静默泄漏 —— 保持这个形态。`AppSpec` derive 了 `Debug` 但没有 `Serialize`，MUST NOT 用 `?spec` / `{:?}` 打印它
- `pm3 restart` 会**重新读盘**（`Supervisor::reload_declaration` 先 `resolver.prepare` 再覆盖 `record.spec`），所以改完 `.env` 用 restart 就生效；`on_restart`（延迟重启）与 `on_fire`（cron）MUST NOT 重读，避免定时任务因文件临时问题失败

### 日志字段

面向 AI 排障，所以字段名比文案重要；改日志前先看这里。

- 每条业务日志 MUST 带 `feature` + `action` 两个字段。**MUST NOT 用 `operation`**：6 必备字段里是 `action`，混用会让按 `action` 过滤的查询整段漏掉（曾有 9 处用 `operation`，`server`/`signal`/`telemetry`/`startup`/`shutdown` 全查不到）。`action` 的值用 `snake_case`，MUST NOT 带点（`drain.start` → `drain_start`）
- `feature` 取值收敛在：`lifecycle` `supervisor` `resurrect` `persistence` `api` `client` `server` `service` `unit`
- 每个**外部调用**（`ps` / `kill` / `launchctl` / `systemctl` / UDS 往返）MUST 记 `duration_ms`：`let started = Instant::now();` 起头，日志里 `started.elapsed().as_millis()`
- 级别按「谁看」分：AI/排障走 `debug`（外部调用、中间状态），人/监控走 `info+`（服务起停成败在 `usecases` 的 `start_one` / `request_stop` 里发）
- **CLI 进程 MUST 自己装 subscriber**，否则 `feature` 为 `client`/`service`/`unit` 的日志全部静默丢弃：装在 `open_session` / `open_service_session`（这两处本来就已读过配置），写 **stderr**（daemon 写 stdout 只因为它被重定向进 `pm3.log`，CLI 的 stdout 是给人看的报文）。曾经 `init_telemetry` 只在 daemon 路径调用，于是 `log_stuck_undo`（服务文件回滚失败 = 盘上文件与运行中的服务不一致）这类**只有日志一条通路**的 warn 从来没人看得到——它不在退出码里、也不在 stderr 文案里
- spawn 日志 MUST NOT 打 `args` 与 `env`：服务的启动参数可能含运维塞进去的凭据
- 「尽力而为」的收尾 IO 可以 `.ok()`，但**改变外部可见状态的失败 MUST 记 `warn`**：`force_kill` 失败意味着孤儿进程存活，服务文件回滚失败意味着盘上文件与运行中的服务不一致。曾经 `UndoStep::apply` 吞掉错误后仍无条件记「回滚成功」——日志说谎比没有日志更糟

### 配置与路径

- daemon 自己的 `config.yaml` 只能放在 `pm3.home`：`cfg_dir` 由配置本身定义，放不进去
- `ensure_layout` 把 `pm3.home` 与 `cfg_dir` 收紧到 `0700`，但 **chmod 失败只 warn、MUST NOT 向上抛**：`cfg_dir` 可以指向配置管理预建（root 属主）或只读挂载的目录，一 `?` 就让**每条 CLI 命令**（`prepared_session`）和 daemon 启动一起失败，而目录权限本来只是「更安全一点」的加固
- **pm3 调用的每个外部程序都来自配置**，代码里 MUST NOT 再出现第二份路径常量：`pm3.service.{launchctl,systemctl,loginctl}_path`（发行版差异大：Debian 在 `/usr/bin`、部分发行版在 `/bin`、NixOS 在 `/run/current-system/sw/bin`）、`pm3.sandbox.{seatbelt,bwrap}_program`（`bwrap` 走 `search_path` 解析，`sandbox-exec` 是绝对路径故 `search_path` 对它无效）。例外只有 `/bin/ps` 与 `/bin/kill`（身份令牌与进程组信号的硬约束，见「进程与信号」）
- `PM3_HOME` 同时决定**配置发现**与 `pm3.home`：`default_config_path` 先读 `PM3_HOME` 再回退 `~/.pm3`。曾经只有 `config.yaml` 里的 `${PM3_HOME:-~/.pm3}` 认它，导致 `export PM3_HOME=/srv/pm3` 后 `pm3 list` 仍去读 `~/.pm3/config.yaml`，每条命令都得带 `--config`
- 读 env 的逻辑 MUST 抽成接 `Option<&str>` 参数的纯函数（`default_config_path(pm3_home_env, home_env)`），env 只在 `frameworks/src/layout.rs` 的 `host_home` / `host_pm3_home` 里读一次：Rust 2024 的 `set_var` 是 `unsafe`，测试无法注入进程级 env
- `substitute_env_vars` **不递归展开默认值**：`${PM3_SEARCH_PATH:-${HOME}/.cargo/bin:...}` 里的 `${HOME}` 会原样留在配置里 → 想让 pm3 找到 `~/.cargo/bin` 下的程序，不要改 `search_path`，直接把服务的 `script` 写成 `${HOME}/.cargo/bin/<prog>`（顶层占位符会展开）
- args 里指代「该服务自己的可写工作目录」MUST 用 `${PM3_SERVICE_CWD}`（命令行写裸 `PM3_SERVICE_CWD`，CLI 折叠成带花括号形式），MUST NOT 写 `${HOME}/.pm3/<name>`（那把 pm3 布局烧进了参数）；只在 args 生效，`cwd`/`writable_roots`/`script` 里写它不展开、会被相对路径校验直接拒；`pm3 describe` 显示的是展开后的真实路径，不能拿它当「配置无绝对路径」的证据
- 服务名 MUST 只含 `[A-Za-z0-9._-]` 且不以 `.` 开头、不能被 `parse::<u32>()` 解析（`entities::validate_app_name`）。校验点在 `service_file_of` **内部**（返回 `Result`）而非各调用方：CLI 是先写盘后交 daemon 校验，只在 `path_safe`（stop/restart/delete/describe）拦一道时，`pm3 start --name ../../../.bashrc` 会先把 yaml 写到 `cfg_dir` 之外、`--force` 还会覆写既有文件。`pm3 logs` 的日志路径同理走 `stdout_log` 的校验：
  - 纯数字会被 `AppSelector::parse` 读成 pm_id，`pm3 stop 3` 会误伤 pm_id=3 的**另一个**服务
  - `/` 与 `..` 会随 `service_file_of` 把服务文件写到 `cfg_dir` 之外（CLI 是先写盘后交 daemon 校验，拦不住）
  - 空格等字符会被原样嵌进 HTTP 请求行，`pm3 stop "my app"` 直接把 request-line 切碎（症状：`the daemon answered nothing`），服务能起却停不掉

## 测试与覆盖率

### 门禁运行（`just cov`，四指标 100%）

- 顺序 MUST 是 `just lint` → `just cov`：`cov` 只跑 nextest 不跑 clippy，`#[expect]` 失效这类问题它看不见；反过来 `cov` 又能暴露 `lint` 漏报的 test target `unused_imports`（clippy 增量缓存可能不重编测试目标）→ 两个都要跑
- **macOS 上 `just cov` 必定非零退出**：`TODO.md` 记着三处平台性基线缺口（`layout.rs` 的 `host_uid`、`ps_probe.rs`、`watcher.rs`）→ 判断自己的改动有没有引入缺口要**逐文件对比那三条**，不是看退出码；出现第四个文件名才是回归
- `cargo-llvm-cov` 忽略路径含 `tests/` 的文件；`test_helpers/` 与 `test_support/` **计入**门禁，helper 里的 `panic!` 会变成未覆盖行
- 改动令行号位移后必须 `just cov --fresh`，否则残留旧实例化产生幽灵 `FNDA:0`
- **全零自救** — 症状：所有文件 0%、`FNDA:0` 上千条；原因：二进制与 profraw 哈希错位（非 fresh 与手动 `cargo llvm-cov report` 交叉跑会触发）；修法：重跑 `just cov --fresh` 且中途不插任何其他 cargo 命令
- **定位 region 缺口** — 症状：`just cov` 失败却一行文件明细都没打（lcov 不含 region 数据，`findFilesBelowFullCoverage` 自然无输出）
  修法：MUST 紧跟在一次 `just cov --fresh` 之后（中途不插其他 cargo 命令）跑 `cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'` 找文件，再 `--show-missing-lines`
  - 无输出且 lines 也缺 → 缺口在 bin 副本（lib+bin 双编译，region 按实例化计数）：补 e2e 走真实 binary，或让分支只存在于一处
  - **按 crate 副本定位**（本轮唯一奏效的办法）：`frameworks` 至少有三份副本同时被计量——lib test、`pm3` bin、以及**每个 `frameworks/tests/*.rs` e2e 二进制各链一份 lib**。一行只被其中一份走到，门禁仍报缺失。定位脚本：`llvm-cov report --release --offline --json` → 取 `data[0].functions[]` 里 `filenames` 含目标文件的项 → 按名字里的 `Cs<hash>_` 分组 → 每组内对 `regions[]`（`[行起,列起,行止,列止,count,fileId,…]`）逐行取 `max(count)` → 哪一组为 0 就得让**那份副本**也走到（lib 侧缺补单测、bin/e2e 侧缺补 `frameworks/tests/` 用例）。本轮 `commands.rs` 的 `Error::UnsavedStart` 分支有 lib 单测仍挂，补一条 e2e 才过
  - 无输出而 lines 100% → 缺的是 `?`/短路的纯 region，重点怀疑新加的 `?`
  - 查完回到 `--fresh`
- **lcov 明细全绿而门禁仍挂** — 症状：门禁报 `lines 382/383, branches 29/30`，但该文件的 `DA:`/`BRDA:` 逐条全非零、`--show-missing-lines` 不出列、`llvm-cov report --text` 里也找不到计数为 0 的行
  原因：`DA:` 是**按源码行合并**后写的（多实例化取并集），而门禁读的 `LF/LH/BRF/BRH` 是 llvm-cov **按函数实例化组**统计后相加、组内取 `max` 而非并集 —— 两份实例化各覆盖一半，`max(1/2,1/2)=1/2` 就报缺失。**解析 lcov 永远查不出来**，`DA` 条数天然少于 `LF`（全仓库每个文件都如此，不是异常信号）
  修法：`cargo +nightly llvm-cov report --release --offline --json --output-path <f>`，按 `data[0].functions[]` 过滤 `filenames` 含目标文件，逐实例化找 `branches[]` 里 `b[4]==0 || b[5]==0` 的项 —— 同一 `line:col` 出现两条、一条只有 true 一条只有 false，就是它

### region 修法

- 每个 `?` 的 Err 分支是独立 region，各需一条失败路径测试；`.expect()` / `.unwrap_or(<常量>)` / `.unwrap_or_default()` 不产生本文件 region，「已证不可达」处用 `.expect()` 优于 `map_err` + `?`
- 但 `.expect()` MUST NOT 出现在**返回 `Result` 的函数**里（clippy `unwrap_in_result`，本仓 `-D warnings`）：想用「已证不可达」的 `.expect()` 收掉一个 `?` 的 Err region 时，把这次调用抽成一个**不返回 `Result`** 的包装函数、在里面 `.ok()`（如 `init_cli_telemetry`）
- 泛型 fn 里的闭包按实例化各算一份 region，测试补不满：`unsaved.map(|error| error.to_string())` 改成 `unsaved.as_ref().map(ToString::to_string)`，消掉闭包即消掉那份 region
- 同一函数里连续两个 `?` 调同一个 helper（`parse_bound(low)?` 后 `parse_bound(high)?`）时，只测「第一个失败」会让第二个的 Err region 永不可达 → 必须再补一条「前者合法、后者非法」的用例（`25~b` 之于 `a~b`）
- `?` 的 Err region 可达性取决于调用顺序：`canonical_config_path` 排在 `load_and_parse_config` **之后**时其 Err 分支永不可达（文件已读成功 → canonicalize 必成功），把「路径解析」提到「读文件」之前才能覆盖
- tail-return（`f().await` 直接作返回值）不产生 Err region，改成 `let x = f().await?;` 就新增一条；收尾处可用 `f().await.map(|x| ...)`（Err 直传不产生 region，closure 是独立 fn 随 Ok 路径覆盖）；真的失败路径则注入依赖让单测能打
- 不可注入的系统读取（`std::env::current_exe()`）不要在函数体里直接 `?`；把 `io::Result<T>` 塞进注入的 context，用 `.as_ref().map_err(...)?` 消费，测试才能构造 Err 命中该 region
- 每个 thiserror variant 都要构造 + `.to_string()` 断言一次，否则该 `Display::fmt` match arm 的 region 不计入覆盖
- `tracing::debug!(field = <表达式>)` 的表达式只在 subscriber 启用时求值，测试无 subscriber → 该行 region 不覆盖；MUST 先 `let x = <表达式>;` 再 `tracing::debug!(x)`
- `if cond { ... }` 块尾的 `}` 会生成一条独立 region，只有「进入块又走完」才算命中；若该路径不可达就改写成 `if !cond { return ... }` 的早返形式
- 轮询循环的 fall-through `}` 不产生计数（`for _ in 0..=n` 尤甚）→ 让函数返回值并在循环后以 `true` 收尾（`while cond { if 超预算 { return false } ... } true`），fall-through 才有可命中的 region
- `tokio::select!` 展开出的不可达 region 无法覆盖；用一个 forward task 把两个 channel 汇成一个，主循环只 `recv()` 一个 queue
- 泛型/`impl Trait` 参数会按实例化各算一份 region：把 `shutdown: impl Future` 改成 `Pin<Box<dyn Future + Send>>` 可把实例化收敛为一份
- 泛型函数里的 `if` 若被两份实例化各走一半（lib 测试只走 true、e2e 只走 false），覆盖率**加多少测试都补不满**（组内取 max，见上一节）→ MUST 把判断抽成**非泛型**纯函数放进 `usecases`，在那里单测两条臂。踩过的坑：`Supervisor::stop_all` 里的 `if covered.contains(&pid)` → 抽成 `query::unswept_pids(tracked, scheduled)`。这类判断本就是业务查询，抽出去顺带把分层也修对了
- 不可达的防御分支应**重写消除**，而非加测试掩盖

### 集成 / e2e 技法

- 被 SIGKILL 的进程已执行行的计数器永不落盘（`cargo llvm-cov show-env` 的 `LLVM_PROFILE_FILE` 含 `%p`，子进程各落一份 profraw，但只在正常退出时写出）→ daemon 集成测试收尾 MUST 发 SIGTERM 并 wait 到退出，否则 e2e 覆盖行丢失
- e2e 收尾 helper MUST 无条件「先 `pm3 list` 拉起 daemon 接管、再 `kill --with-services`」；写成「socket 不存在就 return」会漏掉幸存子进程
- `pm3 __sleep <ms>` 隐藏子命令自身也是生产代码，MUST 有一条「spawn 它、等正常退出、断言退出码 0」的测试；用它而非 `/bin/sh -c sleep` 可摆脱系统 shell 差异
- `sh -c "trap '' TERM; sleep 30"` 在被 pm3 spawn 时并不能可靠忽略 SIGTERM（手工 shell 与 python spawn 都能，pm3 路径不能，原因未查明）→ 不要用它当「顽固进程」测试靶子；要覆盖强杀路径就直接调 `on_force_kill`，或先用假的 `on_exit` 让表以为进程已退出
- 拆超 512 行的测试文件 MUST 把新文件挂成**原测试模块的子 mod**（在旧测试文件末尾写 `#[path = "x_tests.rs"] mod x;`，新文件开头 `use super::*;`）：子模块能看到父测试模块用 `use` 引进来的 helper 与 fixture（`use` 绑定对后代可见），挂在生产文件上做兄弟 mod 则看不到
- 断言「依赖先启动」不能看应用自己写的文件（并发写有竞态），要把 `log_level` 调成 debug 后从 `pm3.log` 里读 `"action":"spawn"` 的顺序
- 断言「子进程环境已清空」MUST 探 `$HOME` 不能探 `$PATH`：`/bin/sh` 在 PATH 缺失时会自己合成一个默认值
- 测「调用外部服务管理器」（`launchctl`/`systemctl`/`loginctl`）用临时目录里的 `#!/bin/sh` 脚本 + `set_permissions(0o755)` 当替身，可同时控制 stdout 与退出码；真实二进制只用 `/usr/bin/true`、`/usr/bin/false`、`/nonexistent/...`，**绝不**在测试里调真的 `launchctl`/`systemctl`
- fixture 里的 `create_dir_all` 会把「测试想要它缺失」的父目录造出来 → 造错误路径的 store/source fixture 必须接一个独立 root，别从被测路径 `parent()` 反推
- `#[tokio::test(start_paused = true)]`（让 `tokio::time::sleep` 自动推进，用来测「定时器到点发事件」）需要在 dev-dependencies 显式写 `tokio = { workspace = true, features = ["test-util"] }`——workspace 的 `"full"` **不含** test-util，否则报 `no method named start_paused`；这种测试里 MUST NOT 用带 `timeout` 的 helper 等事件（timeout 也会被自动推进，可能抢先触发），直接 `events.recv().await`
- 交互询问（confirm prompt）的可测模式：循环签名接 `confirm: &mut (dyn FnMut(&str) -> bool + Send)`，生产传一个「每次调用才锁 stdin/stdout」的 fn（`StdinLock` 非 Send，MUST NOT 跨 `.await` 持有），测试传脚本化闭包；MUST NOT 在单测里碰真 stdin（nextest 下 stdin 是 null → 立即 EOF，且无法注入答案）
- 测试靶子 MUST 写 `sh -c "exec sleep 30"`：漏掉 `exec` 时 sh 只 fork 不 exec，信号打在 sh 上、sleep 成孤儿（症状：nextest 报 LEAK、测试卡满整个 sleep 时长）
- 断言外部命令的错误文案 MUST 跨平台：合法但不存在的 pid 两边都报 `No such process`，而 `illegal process id` 只有 macOS BSD kill 有；需要「真实存在的程序」的测试用 `/bin/sh`，MUST NOT 写 `/opt/homebrew/...`

### 残留清理

- e2e 会泄漏 daemon 与子进程（tempdir 已删、进程仍在）：排查真机状态前先 `pgrep -f 'pm3 daemon --config /var/folders'`（Linux 是 `/tmp/.tmp*`）与 `pgrep -f 'pm3 __sleep'` 各清一遍，否则 `pgrep`/端口结果会误导；子进程自 `process_group(0)` 起不再随测试进程组被连带清理
- 列残留 MUST 用 `pgrep -x pm3` 再逐个 `ps -o pid=,args= -p <pid>` 核对：`pgrep -f` 会把发起它的 shell 一起匹配进来（本轮就误报了两个 pid）。泄漏的 e2e daemon 特征是 `ppid=1` + `--config` 指向已不存在的 tempdir，真机那份指向 `~/.pm3/config.yaml`，别杀错
- **nextest 中断残留** — 症状：flake 触发取消剩余测试 → `TempDir` 的 Drop 跑不到，`$TMPDIR` 留下 e2e fixture 目录（`config.yaml` + `home/{logs,service,pm3.sock}`）
  修法：`rg -l --hidden 'pm3-e2e-never-installed|pm3-fixture' "$TMPDIR" -g config.yaml` 定位
  陷阱：`rg` 默认跳过隐藏目录而这些正是 `.tmp*`，漏 `--hidden` 会得到假阴性；按 label 指纹而非目录名匹配，才不会误删真机配置

### 测试代码的 clippy

- 同一 `test_support/*.rs` 文件 MUST NOT 被两处 `#[path]` 重复挂载（clippy `duplicate_mod`）；统一在 `lib.rs` 以 `#[cfg(test)] pub(crate) mod` 挂载一次
- test_helper 的请求构造器 MUST NOT 与 handler 同名（`get`/`post`/`delete` 在 `use super::{test_helpers::*, *}` 下二义），用 `get_from`/`post_to`/`delete_at`
- 只有 `Ok` 分支的测试 fixture 会触发 clippy `unnecessary_wraps`：fixture 返回裸值，调用处再 `Ok(...)`

## 外部工具与库的坑

- clippy 会报 `similar_names`（`launcher` 与 `launched`、`receiver` 与 `received`）、`shadow_unrelated`（闭包参数名与外层 `let` 撞名即报，换个名字即解）
- `elidable_lifetime_names`：`fn f<'s>(x: &'s [T]) -> R<'s>` → `fn f(x: &[T]) -> R<'_>`
- clippy `string_slice` 禁掉一切 `&text[n..]`，哪怕 `n` 是 ASCII 常量的 `.len()`：扫描字符串用 `split_once(pat)` 循环，别 `find` + `split_at` + 切片（`strip_prefix` 也别用，它多一条永不可达的 `else` 分支，覆盖率补不上）
- `.collect::<Vec<_>>().join("")` 触发 clippy `unnecessary_join` → 改 `.collect::<String>()`
- clippy `format_push_string` 与 `format_collect` 互相堵死：`push_str(&format!)` 和 `.map(format!).collect::<String>()` 都报，出路是 `fold(format!(init), |mut t, x| { let _ = writeln!(t, ..); t })`，或把闭包体抽成**具名** fn 再 `.map(named).collect()`（`format_collect` 只盯闭包体是 `format!` 的形态，`launchd.rs` 的 `render_argument` 就是这么写的）
- 跨 async 边界的回调参数要写 `&(dyn Fn(&str) + Send + Sync)`，否则外层 future 不是 `Send`
- 结构体从「拥有」改成「借用配置」后，返回 `Foo<'static>` 的 fixture 会编译失败 → 用 `LazyLock<Config>` 让引用变 `'static`
- axum 0.8 原生 `impl Listener for tokio::net::UnixListener`（无需 hyper-util）；`tokio::net::unix::SocketAddr` 只 impl Debug 不 impl Display → 日志用 `?addr`
- clap `trailing_var_arg` + `allow_hyphen_values`：pm3 自身选项必须出现在程序名**之前**，否则被当子进程参数
- **Rust 生态没有任何 cron 库支持 OpenBSD 风格的随机 `~`**（croner/cron/cronexpr/jiff-cron/cron_tab 全无，只有 cronexpr 支持 Jenkins 的固定哈希 `H`）→ 自己展开成具体数字再交 croner
- 判「是不是 OOM」用 `/proc/vmstat` 的 `oom_kill`（开机以来内核 + cgroup OOM 累计杀进程数）：为 0 即可彻底排除，比翻 dmesg/journal 可靠
- 抓「谁杀了进程」MUST 用 `sudo systemd-run --unit=X --collect perf record -a -e syscalls:sys_enter_kill -e signal:signal_generate`：直接从用户会话起的 perf 属 `user-1000.slice`，slice 一崩它就陪葬、数据废掉（`data size field is 0`）；输出里行首是发送者、`comm=`/`pid=` 是目标、`grp=1` 表示进程组广播
- 安全验证 kill 语义用 `strace -e trace=kill /bin/kill -0 -- <target>`：sig 0 只探测不投递，能看到内核实际收到的 pid
