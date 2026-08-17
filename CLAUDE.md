@~/.claude/CLAUDE.md

@~/.claude/clean-arch.md

@~/.claude/rust-p0.md

@~/.claude/rust-p1.md

pm3：极简版 pm2（带严格沙盒隔离）。单二进制，CLI 与常驻 daemon 合一，经 Unix socket（Windows 为命名管道）通信。

文档分工：`README.md` 只给用户讲怎么用；`docs/requirements.md` 讲要什么行为（给人看，不含技术细节）；`docs/windows.md` 是 Windows 能力矩阵；**CLAUDE.md 只记踩过的坑——跨层的记在本文件，单层实现细节下沉到各 crate 的 `CLAUDE.md`。** 新增内容前先按这个分工挑文件，别往 README 或需求里塞实现细节。

## 项目速览

| crate | 职责 | 层内坑 |
|---|---|---|
| `entities` | 业务对象与状态机：`AppSpec`/`ProcessStatus`/`RestartPolicy`/`DepGraph`/`SandboxPolicy` | `entities/CLAUDE.md` |
| `usecases` | Interactor 与 Port trait：`supervisor`（daemon 编排总入口）/`start`/`stop`/`restart`/`delete`/`resurrect`/`supervise`/`query` | `usecases/CLAUDE.md` |
| `adapters` | 格式转换与外部实现：config/http/persistence/presenter/process/sandbox/schedule/service/unit | `adapters/CLAUDE.md` |
| `frameworks` | 组装与入口：`main.rs`/`cli.rs`/`daemon/`/`client/`/`service.rs`/`signal.rs` | `frameworks/CLAUDE.md` |
| `arch_tests` | 依赖方向强制 | `arch_tests/CLAUDE.md` |
| `dev_scripts` | Bun/TS 驱动 `just` 的复杂 recipe；覆盖率门禁与残留 reap | `dev_scripts/CLAUDE.md` |

全部 recipe 见 @justfile，四个要点：`just cov` 是**日常验收**（四指标 100% + lcov 真值 plate + 生产文件完整性自检，`--fresh` 清 workspace 重算）；`just lint` 是 clippy 四组全开、任何 warning 即失败；`just fmt` 必须 nightly（只有它能重排 import）；`just install` 才是装真机的唯一入口。另有 `check-windows`（交叉编译检查，只编译不链接）、`monitor`、`bench`、`typecheck`、`test-scripts`。

## 命令与工作流

- 优先用 @justfile，禁止随手 `cargo`；例外：`just` 的 recipe 都是 workspace 级，单 crate 迭代用 `cargo <cmd> -p <crate> --release --offline`
- 改过 `Cargo.toml` 后 `--locked` 会直接失败，改用 `--offline`
- Bash 命令里出现字面量 `.env` 会被 hook 整条拦掉（报 `禁止读写 .env 文件`），而本仓到处是 `spec.env` / `entry.env` / `<name>.env` → 查这些内容用 Read / Grep 工具，别用 `rg`/`grep`/`sed` 的 Bash 命令；真机 `cfg_dir` 下的凭据文件连 Read 也被权限设置拒掉 → 验收「`.env` 生效了没」只能看日志与服务状态，别把「读一眼那个文件」写进步骤
- 手工验证要另建 pm3 home：scratchpad 路径太长，unix socket 会撞 macOS `SUN_LEN`（>104 字节，报 `path must be shorter than SUN_LEN`）→ 用 `mktemp -d`
- 改 `adapters/` 的目录结构、重命名类型、或把常量下沉进 `config.yaml` 后 MUST 跑 `just test-scripts`（原因见 `dev_scripts/CLAUDE.md`）

### 装真机与换代

- 固定走 `just install`（构建后调 `pm3 install`），MUST NOT 手工搬二进制（换代顺序有硬约束，见下）。`pm3 install [SOURCE]` 自己走完：备份 → `.incoming` + rename 换二进制 → `service uninstall` → `kill` → `service install --force` → 等接管 → before/after 对比（lost 非空即非零退出）。换代步骤在**进程内**完成，不 spawn 新二进制：rename 覆写正在执行的文件在 macOS/Linux 合法（ETXTBSY 只在 write-open；Windows 不能覆写运行中的 exe，两步 rename 见「Windows」节），且 `ServiceContext.binary` 显式注入 destination——rename 后 `current_exe()` 在 Linux 会带 ` (deleted)` 后缀，流程内 MUST NOT 再读它
- 「等 daemon 退净」不靠 `pgrep -f "<bin> daemon"`：`kill_daemon` 内置 `wait_until_released(socket)`（daemon 收尾先删 pid 再删 socket，socket 消失即清理完毕），比 pgrep 准且少一个外部程序
- before/after 服务对比直读 `dump.yaml`（`dump_snapshot`，只取 name+pid，不 resolve spec），MUST NOT 调 `pm3 list`——那会经 `ensure_daemon_running` 拉起非托管 daemon 抢 socket；after 快照由「接管等待」收敛（Running + 管理器 pid == `pm3.pid` + UDS 健康三者同时成立，因为 pid 文件先于 resurrect 写入，单靠 pid 对齐不算接管完成）
- 排查真机先从 `~/.pm3/config.yaml` 读 `cfg_dir`：它与 `pm3.home` 各自独立配置（本机 `home=~/.pm3`、`cfg_dir=~/.config/pm3`、二进制在 `~/bin/pm3`），按 `<home>/service` 猜必定扑空
- `pm3 service install` 用 `current_exe()` 渲染 unit → MUST NOT 在仓库目录执行（会把 plist 钉在 `target/release/pm3`，一次 `cargo clean` 就起不来）；先把二进制 `cp` 到最终位置，再用**那个**二进制执行 install
- **症状**：`launchctl list` 的 PID 列是 `-`，job 已载入但 launchd 未监管、KeepAlive 形同虚设
  **原因**：任何 pm3 CLI 命令都会经 `ensure_daemon_running` 自动拉起一个**非 launchd 托管**的 daemon；它扛不住 `launchctl unload`，且会抢赢 socket 竞争让 launchd 那份直接退出
  **修法**：换代顺序 MUST 是 `service uninstall` → `pm3 kill` → 等 daemon 退净 → `service install --force`（`pm3 install` 内建这一整套）；install 后 MUST 等「launchd 报的 pid == `pm3.pid` 内容」再跑任何 CLI 命令，否则又会拉起竞争者。已处于未监管态时先 `pm3 kill` 停掉自启实例，再 `launchctl kickstart gui/$(id -u)/<label>` 交回 launchd（`pm3 install` 在 launchd 超时后自动做一次 kickstart 重试）
- unit MUST 导出安装时的 `PM3_*` 环境（`UnitSpec.pm3_env`，三个渲染器都写，Windows 写进 `.cmd` 包装脚本；值要排序否则 `reconcile` 的逐字节比对每次都判 Stale）：install 拷进 `pm3.home` 的 `config.yaml` 是**未做变量替换**的原文，而 unit 只导出 `HOME`/`PATH` 时，`${PM3_HOME:-~/.pm3}` 在服务管理器起的 daemon 里退回默认值 ⇒ daemon 在 `~/.pm3` 建 socket/pid/dump，而 CLI（shell 里有 `PM3_HOME`）去连 `/srv/pm3/pm3.sock` ⇒ 连不上就经 `ensure_daemon_running` 再拉起一个**非托管** daemon，正是本节开头那个坑。注意反过来把「替换后的文本」落盘会让 `reconcile` 每次 install 都判 Conflict
- 换代备份落在 **`<pm3.home>/install-backups/<旧版本号>/`**（`adapters::install::backup_root`，`PM3_INSTALL_BACKUPS` 可覆盖，目的地 `PM3_INSTALL_PATH`；目录名取 `<旧二进制> --version` 的末位 token，查不到即 `unknown`），MUST NOT 放回 `~/.pm3-install-backups`：备份里有旧 `config.yaml`，而 `mkdir` 的权限受 umask 摆布（实测出过 0775）→ 放进 `pm3.home` 才被那层 0700 兜住（备份目录与文件再显式 chmod 0700/0600），且它正好是沙箱的 hidden root ⇒ 被托管的服务连备份都看不见。回滚就是从对应版本目录取 `pm3` 二进制 + unit + config 三件套
- 手工 `cp` 二进制会撞 `Text file busy`（旧 daemon 还在跑）→ 先 uninstall + `pm3 kill` 再拷。`pkill -f '<path> daemon'` 会匹配到发起它的 shell 自身命令行、把自己一起杀掉（症状：命令 exit 144），列残留的方法见 `dev_scripts/CLAUDE.md`
- Linux 侧同一套顺序换 `systemctl --user`。`XDG_RUNTIME_DIR` 与 linger 的三个坑（都不需要手工 export，但手工敲 systemctl 排查时要）见 `adapters/CLAUDE.md`「服务管理器」

### Windows（Task Scheduler + 命名管道）

能力矩阵与降级清单见 `docs/windows.md`，这里只记跨层的坑。

- 服务形态是当前用户 OnLogon 任务（免管理员）：unit 是 Task 2.0 XML，落在 `~/.pm3/service/<label>.xml`，经 `schtasks /Create /XML` 注册。**Task Scheduler XML 不支持环境变量** → `HOME`/`PATH`/`PM3_*` 由同目录的 `<label>-daemon.cmd` 包装脚本逐行 `set`（值里 `%` 转义成 `%%`），脚本末尾恒 `exit /b 1` —— 这是 restart 语义的关键：Task Scheduler 只在「失败」时重启（RestartOnFailure，最小间隔 1 分钟），恒报失败 ≈ `always`，代价是 `on-failure` 在 Windows 降级为 `always`
- **tokio 在 Windows 没有 `UnixListener`**（std 有但只能阻塞，进不了 reactor）→ 传输在 Windows 是命名管道 `\\.\pipe\pm3-<hash>`（`layout::pipe_name_of`，`DefaultHasher(socket 路径 + secret)`；CLI 与 daemon 同二进制，哈希一致即可），而 `pm3.sock` 仍存在但只是**存在性标记文件**：`bind_uds` 建管道后写标记、`clear_runtime_files` 删标记，`wait_until_released` 与「stale socket 自愈」全部照旧走文件。MUST NOT 把标记文件当成真 socket 去 bind
- **管道名 MUST 混入 `<pm3.home>/pipe.secret`**（`layout::pipe_secret`，缺失即生成、损坏即重建，0600 语义靠 NTFS 用户目录）：Windows 无 `SO_PEERCRED`，peer 准入是 fail-open，「别的用户猜不出管道名、也抢不到注册」是这里唯一的隔离手段。只哈希 socket 路径的话管道名对同机任何用户都可推算 ⇒ 抢注即中间人
- Windows 上**不能覆写正在运行的 exe**（rename 上去报 AccessDenied，与 ETXTBSY 不同）→ `replace_binary` 先把旧 exe rename 成 `<destination>.retired`（运行中的 exe 可以改名、不能删不能盖），再把 `.incoming` rename 到位
- `UnitKind::WinSchtasks` 变体、XML/wrapper 渲染器、schtasks 命令构造器 MUST NOT 加 `cfg(windows)`：纯逻辑跨平台编译，单测全在 Linux 跑（与「systemd 渲染器在 macOS 可测」同模式）；`#[cfg]` 只出现在宿主事实采集（uid/HOME/信号/权限位）与传输层。这样 `just cov` 门禁对 Windows 代码天然免疫
- 信号/强杀的 Windows 对应：`/bin/kill` → `taskkill /PID <pid> /T /F`（/T 杀进程树 ≈ 进程组；TERM 与 KILL 无差别，优雅停机只有 daemon 自己的 CTRL_SHUTDOWN 落盘）；`.process_group(0)` 与 `peer_cred()` 只编译在 unix，socket 准入在 Windows 走 fail-open（与「读不出属主即放行」同设计）

### 资源上限（fork bomb 防线）

`RLIMIT_NPROC` / seccomp 要 `libc` + `unsafe`，与 workspace 的 `unsafe_code = "deny"` 冲突 → 防线改由**服务管理器**声明，零 unsafe、零新外部程序，且是内核级的硬限制。两项都只在 unit 文件里，**改配置后 MUST `pm3 service install --force` 重装 unit 才生效**，光 `pm3 kill` + 重启 daemon 没用。

- `pm3.service.max_tasks` → systemd `TasksMax=` / launchd `HardResourceLimits.NumberOfProcesses`。**它是 pm3 整体的总量**（daemon + 全部被托管服务共处一个 cgroup / 同一 uid），不是每服务限额 → 调低到「够正常用」的值会在服务数量涨上来时集体 spawn 失败，默认 4096 是按此留的余量。**systemd 数的是 task 不是进程**，JVM/Node 这类多线程服务吃得比看起来多
- `pm3.service.cpu_quota_percent` 只渲染进 systemd（`CPUQuota=`，可超 100% 表示多核），**launchd 侧刻意不渲染**：它只有 `RLIMIT_CPU`（累计 CPU 秒数），到点直接杀进程 ⇒ 一个健康的长跑 daemon 必被杀。默认 `0` = 不限制，因为盲目限 CPU 会拖慢正常的计算密集服务；fork bomb 的爆炸半径已由 `max_tasks` 兜住

## 改动波及清单

改这里就必须同步那里，漏一处即编译失败或运行期对不上。

- 给 `Pm3Config` 加字段要同步 6 处：根 `config.yaml`、`adapters/test_support/config_sections.rs`、`adapters/src/test_helpers/config_schema_test_helpers.rs`、`frameworks/test_support/config_fixtures.rs`、`frameworks/tests/common/mod.rs`、校验函数与 `every_error_variant` 表
- 给 `UnitSpec` 加字段要同步 6 处：`adapters/src/unit/spec.rs` 的结构体、`launchd.rs`/`systemd.rs`/`schtasks.rs` 三个渲染器、`adapters/test_support/unit_specs.rs` 的 `spec_for`、`frameworks/src/service.rs` 的 `build_spec`（唯一生产构造点）；渲染器漏一个不会编译失败，症状是只有那个平台的 unit 少字段而其他平台测试全绿 → 新字段 MUST 在每份渲染器测试里各断言一次（某平台没有对应能力时，就断言「它不出现」）
- 给 `UnitProgramSet` 加字段要同步 4 处：`adapters/src/unit/command.rs` 的结构体与 `from_config`、`adapters/test_support/unit_specs.rs` 的 `program_set`、`frameworks/src/service.rs` 的 `ServiceContext` 与 `open_service_session`、`frameworks/src/tests/service_tests.rs` 的两处字面量；宿主派生值（uid、`XDG_RUNTIME_DIR`）经 `ServiceContext` 注入（读 env 纪律见「配置与路径」）
- 给 `SpecSource` 加字段要同步 4 处：`adapters/src/apps_file/source.rs` 的结构体、`frameworks/src/daemon/service.rs`（唯一生产构造点）、`adapters/test_support/spec_sources.rs`、`frameworks/src/test_helpers/daemon_actor_test_helpers.rs` 与 `frameworks/src/tests/daemon_ports_tests.rs`；宿主派生值（`host_home`）同样只在 `layout.rs` 读 env 后注入
- 改 `usecases/src/ports/*` 里 trait 方法的签名要同步 3 个实现：`adapters` 侧的真实现（如 `YamlDumpStore`）、`frameworks/src/daemon/ports.rs` 的 `DaemonPorts` 转发、`usecases/src/test_helpers/ports_test_helpers.rs` 的 `FakePorts`；`FakePorts` 新增的 `seed_*` 若没人调用会触发 `dead_code`，且它计入覆盖率门禁
- 给 `ExitOutcome` 加变体要同步 4 处：`usecases/src/ports/launcher.rs` 的 enum 与 `failed()`、`adapters/src/process/tokio_launcher.rs`（子进程）、`adapters/src/process/watcher.rs`（adopt 来的进程）、`frameworks/src/daemon/timers.rs` 的 `unwrap_or`
- 给 `ProcessRuntime` 加字段要同步 4 处：`adapters/src/persistence/dto.rs` 的 `RuntimeDto` + `decode_state` + `encode_state`（两处都穷举解构）、`adapters/test_support/process_records.rs`；跨版本可读的字段一律 `#[serde(default)]`
- 给 `ProcessTable` 加「非记录」字段（如 `boot`）要想清**谁负责填**：`from_records` 一律重建，所以 `resurrect` 里 `*table = ProcessTable::from_records(..)` **之后**才能 `remember_boot`；`save_table` 从表里取值，这样各 usecase 的 `save_table(table, ports)` 调用点一处不用改
- 给 `SandboxPolicy` 加字段会波及 ~13 处字面量、给 `AppSpec` 加字段会波及 ~9 处（四层的 test_helpers/test_support）→ 加完先 `cargo build --workspace --all-targets` 靠 E0063 逐个补齐，字段插在字面量开头即可（顺序无关）
- 落盘 MUST 走 `adapters::write_private` / `append_private`（`0o600`，`private_file.rs`）：`tokio::fs::write` 与裸 `OpenOptions` 把权限交给 umask，而 pm3 从不设 umask。`.mode()` 只在**创建**时生效，已存在的旧文件权限不变——真机升级后的旧 `dump.yaml` 仍是 0644，靠 `pm3.home` 的 0700 兜底
- 新增「含路径的字段」或新增 `${...}` 占位符各只有一个改点（`adapters::fold_entry` 与 `substitute_env_vars` 的保留名分支）——细节见 `adapters/CLAUDE.md`

## 领域不变量

跨层生效的业务规则；单层实现细节见各 crate `CLAUDE.md`。

### 进程与信号

- 停止/强杀 MUST 先对进程组发信号（`/bin/kill -<SIG> -- -<pid>`）、失败再退回单 pid：spawn 时 `process_group(0)` 让子进程自成组，只杀单 pid 会漏掉它 fork 的孙进程；adopt 来的进程可能不是组长，故回退分支必须保留
- 传给 `/bin/kill` 的 pid MUST 先过 `is_signalable`（`pid >= 2 && i32::try_from(pid).is_ok()`）：procps 对超 i32 的值**静默截断**——`4294967295` → `kill(-1)` 杀光当前用户所有进程（user manager/tmux/全部用户级服务一起没），`-4294967295` → `kill(1)`；macOS 的 BSD kill 严格报错，故此坑只在 Linux 炸。上一条的「组信号失败即回退单 pid」会把第一步的失败直接放大成第二步的灾难
- `Stopping` 不是「已停止」：判「pm3 是否还持有进程」用 `ProcessStatus::is_settled()`（仅 Stopped|Errored），用 `!is_running()` 会让重复 `stop` 清空 pid、让 `restart` 再 spawn 一个同名实例
- SIGTERM 只落盘退出、不停服务，彻底停机只有 `pm3 kill --with-services`
- daemon 换代（shutdown）MUST NOT 把 `Stopping` 记录改写成 `Stopped`：`persist_for_handover` 只落盘、保留状态与 pid。改写会清掉 identity，让下一代 `resurrect` 的 `!is_settled()` 筛选整条跳过它 —— 既不 evict 也不监控，drain 未完的进程永久残留，随后一次 `start` 就起出第二份实例。对应地 `resurrect` 对 `Stopping` 记录走 `Verdict::Settle`：先 `evict` 掉幸存者再记 `Stopped`（把上一代没做完的 stop 做完），MUST NOT 让它走 `Adopt` —— 那会把运维明确停掉的服务重新拉回 `Online`
- 所有强杀入口 MUST 共用一条带守卫的路径（`Supervisor::sweep_pid`：查 `tracked_pids` → `pid_was_recycled` 比对 token → `force_kill`，失败记 warn）。新增清扫入口 MUST 复用 `sweep_pid`，MUST NOT 在 `frameworks` 层自己调 `ports.force_kill`——裸发信号不校验 token，pid 复用后会打掉无关进程组（Linux 上尤其致命）
- 「延迟重启」在途期间 MUST 可被 `stop`/`delete`/`stop_all` 取消，否则 `restart_delay_ms` 窗口内被停掉的服务会自行复活
- generation 守卫的两条铁律（`on_force_kill` 有 token 时让路、`delete` MUST NOT 清 generation）与 `stop_all` 的编排纪律见 `usecases/CLAUDE.md`「停止与强杀」——这些坑的共同症状都是「服务停了又自己回来」或「强杀打到复用 pid」
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
- `evict_pid` MUST 在发信号**之前**再探一次 `identity(pid)` 并比对 token：verdict 用的是 `judge_all` 开头那一批 `identities()`，而各服务的驱逐（每个最多等满 `kill_timeout_ms`）会让排在后面的服务 token 严重过期 ⇒ 首次 `terminate(-pid)` 就可能打掉复用 pid 的无辜进程组（`wait_gone` 之后那道 `pid_was_recycled` 只兜得住后续 `force_kill`）。全部驱逐经 `evict_all` 用 `join_all` **并行**执行（原来逐服务串行，缺 `procps` 时 20 个服务要阻塞 ~32 秒，期间所有 CLI 命令挂起到超时）
- **跨过一次主机重启，dump 里所有 pid 一律作废**（`PidTrust`）：boot 标识取 **pid 1 的 `lstart`**（`ports.identity(1)`，systemd/launchd 都在 boot 时启动），存 `dump.yaml` 顶层 `boot:`。这样不必引入新 Port、新外部程序或 `libc`——`ps` 本就是硬依赖。`PidTrust::Lost` 时 `judge` 直接 `Change::Rebooted` 且 `stale: None`，`surviving_pid` 提前返回 `None` ⇒ 全程不对陌生 pid 发信号。**两个 fail-safe 方向 MUST 保住**：dump 无 `boot`（旧版升级上来）或本机读不出 pid 1 都判 `Kept`，退回原有的 token 校验，MUST NOT 反过来「未知就作废」（那会让每次升级都 evict 全部服务）

### 沙箱

- **自带沙箱的程序 MUST 用 `mode: danger-full-access`**：seatbelt 不允许嵌套，sshd 的特权分离子进程调 `sandbox_init` 建自己的沙箱时被拒 → 日志 `sandbox initialization failed: Operation not permitted` / `ssh_sandbox_child: sandbox_init: Operation not permitted [preauth]`，症状是端口监听正常但握手就断（客户端看到 `kex_exchange_identification: read: Connection reset by peer`）。换 `read-only` 之类无用（任何 profile 都拒），OpenSSH 7.5+ 也已移除 `UsePrivilegeSeparation no`。`adapters/src/sandbox/wrapper.rs` 对 `is_unconfined()` 直接返回未包装命令，等于恢复 launchd 时代的约束强度——不是降级。同理 macOS app bundle 也套不进去（Google Drive 靠 setuid root 的 `mount_helper` 挂载盘符）
- `pm3 start` **没有设置 mode 的命令行开关**（开关全表见 README，沙箱相关只有 `--network` / `--writable-dir` / `--readable-dir`）：非默认 mode 只能改 `cfg_dir/<name>.yaml` 再 `pm3 restart`（restart 会重新读盘）

`SandboxPolicy` 的路径分四类（读只有 `full|minimal` 两档，刻意不做最长前缀裁决表）：

| 字段 | 来源 | 进指纹 |
|---|---|---|
| `read` / `readable_roots` | 运维声明 | 是 |
| `writable_roots` | 运维声明 | 是 |
| `derived_roots` | pm3 推导 cwd/logs/tmp | 否 |
| `unreadable_roots` | pm3 注入 `pm3.home` 与 `cfg_dir` | 否 |

- **默认 `read: minimal`**：`--tmpfs /` 打底后只铺 `pm3.sandbox.minimal_read_roots` + 声明的 `readable_roots` + **程序自身的路径**（漏了最后一条 exec 直接 ENOENT）。服务报 EACCES 时先补 `readable_roots`，退路是该服务写 `read: full`
- **`unreadable_roots` 与「可写根」的嵌套方向是安全语义，不是风格**：bwrap 是 `--tmpfs <hidden>` → `--bind <granted>`（最浅的先）→ **再** `--tmpfs` 那些落在 granted 之下的 hidden（`nested_in`）；seatbelt 无顺序语义，hidden 靠 carveout 实现，且 MUST 按 `(granted, hidden)` 配对生成——只有 `covers_path(granted, hidden)`（hidden 嵌套在 granted 之内，含相等）才给那条 allow 挂 `(require-not (subpath (param "HIDDEN_i")))`。无条件给每条 allow 挂全部 hidden 的 carveout 会把「granted 嵌套在 hidden 之内」的授权整条作废（`require-not` 命中祖先即整条规则不成立，`deny default` 兜底 ⇒ cwd 既不可读也不可写），而 cwd 默认就在 `pm3.home` 下——PM3-44 曾因此让所有服务一重启就 EPERM；反方向本就由 `deny default` 兜住，不需要 carveout
- **任何可写根都 MUST NOT 覆盖 hidden root**（`validate_policy` 拒绝，含 `derived_roots`）：`cwd: <pm3.home>` 会把 socket 与全部 `.env` 一起交回给服务，两种后端都救不回来——测试 fixture 因此一律用 `<home>/work` 而非 `<home>` 当 cwd
- `network: true` 只放行 IP，MUST NOT 连 unix socket 一起放行（macOS 上那等于把 `pm3.sock` 交给服务）

两个后端各自的 profile/参数纪律（SBPL 写法、DNS 放行、GPU 放行、bwrap 的 namespace 与 `--new-session`）见 `adapters/CLAUDE.md` 的「沙箱与路径」。

### cron 调度

- 到点只调 `restart_app`、**不新增状态**（架构照抄 pm2 `lib/Worker.js`）
- `Fire` 事件 MUST 先比对 `timers.get(name).fire_at_ms == Some(fire_at_ms)` 再执行，否则已过期的定时器会误触发；`stop`/`delete`/`stop_all` 三条路径 MUST 走 `disarm`（remove + `JoinHandle::abort`）——`Daemon.timers: HashMap<name, Timer>` 同时是 `next` 列数据源、调度激活标记与过期判别依据，这样 `next` 有值即「等触发」、空即「真停了」，避开 pm2 那句 `stopped but CRON RESTART is still UP` 的语义混淆
- `Timer` MUST 持 `JoinHandle` 并在重新 `arm_timer` 时 abort 旧 task：只存 `fire_at_ms` 会让每次 restart 多留一个睡到旧 deadline 的孤儿 task
- 「这个服务的调度是否激活」MUST 落盘（`ProcessRuntime::schedule_armed`）：只存在内存 `timers` 里的话，daemon 换代后 `arm_known_timers` 会把用户 `stop` 掉的 cron 服务重新武装、到点自行复活

### 内存熔断

- 采样 MUST 走**独立一条** `ps`，MUST NOT 往身份令牌那条（`pid=,lstart=`）加列：令牌解析把第一个空格之后的整段当值，加一列 rss 会让每次内存波动都被判成 pid 复用、全部服务在换代时被驱逐（三条 `ps` 的分工见 `adapters/CLAUDE.md`）
- tick 是自持循环（`SupervisionEffect::ScheduleMemorySample` → `DaemonEvent::SampleMemory` → 处理完再排下一次），**无条件续排**、只在「没有服务声明限额」时跳过 `ps`：这样不必跟踪「tick 是否在途」，start/delete 也不用管重新武装。**首拍由 `serve_supervised` 在 resurrect 之后注入**（`events.send(SampleMemory)`，日志 rotate 的 `RotateLogs` 同处）——没有首拍整条链在生产上永不启动而单测全绿（单测直接调 handler）；新增自持 tick 时 MUST 在同一个注入点登记
- `max_memory_kib` **不进指纹**（`fingerprint.rs` 里标 `_`）：限额是运维策略不是进程身份，改限额不该 evict+respawn
- 超限只调 `restart_app`（与 cron 同一条路径），所以 `restart` 是异步的——测试断言要看 `stopping` 状态或事件，不能直接比对新旧 pid

### 就绪探针

- 带 `ready_probe` 的服务 spawn 后停留 `Launching`，`TaskBoard` 的探针 task 报 `Ready`/`ReadyTimeout` 事件才推进；adopt 来的 `Online` 进程直接 Online，MUST NOT 重新等探针
- 探针在**宿主机、沙箱外**执行（exec 探针经宿主 `search_path` 解析、tcp 探针测客户端视角可达性；沙箱内 `network: false` 的服务也能被探到）；探针命令 MUST NOT 进日志（凭据规则同 spawn 参数）
- 就绪等待是异步的，所以 `start` 回复在探针出结果前已发出：`Launching` 与被依赖挂起的 `Deferred` 都 MUST 算正常 outcome、MUST NOT 进 `refused`（否则 CLI 会把服务文件回滚删掉）
- 探针超时是**终态**：terminate + `ready_failed` 标记，随后的退出事件直接记 `Errored`，MUST NOT 走 `decide_restart`（不触发 autorestart）；等待中的依赖者级联标 `Errored`
- 已知限制：探针窗口内 daemon 换代，Deferred 服务以 `Stopped` 落盘、不自动续拉（无孤儿无双开，需人工 `start`）；waiters 只在内存，不落盘
- adopt/waiter/取消三处的实现约束见 `usecases/CLAUDE.md`「就绪探针」

### 日志

- 服务日志 fd 由子进程独占（daemon 以 O_APPEND 打开后 move 给子进程）⇒ rename 式 rotate 无效，写侧只能 **copytruncate**；`pm3.log_rotate_max_bytes` 默认 0 = 关闭
- 读侧两条路径（`read_tail` 与 `LogFollower`）MUST 受 `pm3.log_read_max_bytes` 约束：没有上限时，服务用 `\r` 刷进度条会让 `pm3 logs -f` 的内存无界增长，而对一个无换行的巨型日志执行 `pm3 logs` 会直接 OOM（rotate 只按字节切，不保证有换行）
- `pm3 logs` 全程不经 daemon（服务名从 `cfg_dir` 枚举、流选择与行前缀都在 CLI 侧，见 `frameworks/CLAUDE.md`）

### CLI ↔ daemon 协议

- 是 JSON envelope `ReplyDto { report, service, already_running, refused, unsaved, views }`：新增命令走 `ask_report`（只要文案）或 `ask`（要结构化字段）；MUST NOT 靠 `.contains(渲染文本)` 反解业务状态。`views` 是 `ProcessViewDto`（adapters 侧 DTO，白名单字段天然不含 env），只有 list/describe 填；`--json` 由 CLI 拿 `views` 调 `render_json_*` 本地渲染，frameworks 不许碰 serde_json
- `start` 是**部分提交**的批处理，所以回滚粒度 MUST 与提交粒度一致：daemon 在「起了一部分」时回 200 + `refused`（未起来的服务名），CLI 只 `undo.run_for(&refused)` 并以 `Error::PartialStart` 结束（非零退出）；一个都没起来才回非 200、CLI 全量回滚。把部分成功当成「什么都没发生」而全量删服务文件，会让已在跑的服务下次 daemon 启动时 `rejoin` 失败被丢弃 → 永久孤儿
- `start` 的「某个服务起不来」与「起来了但落不了盘」MUST 是两个独立字段（`refused` / `unsaved`）：`refused` 由「requested 减 outcomes」反推，天然表达不了「全都起来了但 `dump.yaml` 写失败」⇒ 回 200 + 空 `refused` ⇒ CLI 退出码 0，而 dump 里没有这些服务，下次 daemon 重启 `resurrect` 读不到记录、既不 evict 也不监控 ⇒ 永久孤儿，CI 按退出码判定完全无感。`unsaved` 非空时 CLI MUST 非零退出（`Error::UnsavedStart`）且 MUST NOT 回滚服务文件（服务在跑）。`Supervisor::start` 只在 `outcomes` 为空时才返回 `Err`
- `start` 请求只传服务名列表（`services: Vec<String>`）——服务文件是唯一事实来源，MUST NOT 把 spec 塞进请求体
- 但**手写 `cfg_dir/<name>.yaml` 不足以让服务被认出**：`pm3 start <name>` 对不在 daemon 服务表里的名字会退回按 apps 文件解析，报 `cannot resolve the apps file '<name>'`。新服务 MUST 先用 inline 形式注册一次（`pm3 start --name <name> [--network] <prog> [args]`），此后 `<name>` 才能直接用；pm3 写出的 yaml 可与手写的逐字节相同，差别只在注册路径
- CLI MUST 在请求头带 `x-request-id`（`adapters::REQUEST_ID_HEADER`，值取 `<CLI pid>-<序号>`），daemon 的 `request_id_of` 优先读它、缺失或空才回退内部 `AtomicU64`。回退分支 MUST 保留：换代期间旧版 CLI 不发这个头。少了这层，一次 `pm3 start` 的客户端日志与 daemon 日志无法串起来（daemon 侧计数器每次重启从 1 开始，跨进程毫无意义）
- **连进来的 peer 要过 uid 校验**（`OwnerOnlyListener`，`SO_PEERCRED` 经 `UnixStream::peer_cred`）：owner 取**socket 文件自己的属主**（本进程刚创建它，macOS/Linux 都拿得到，无需 `/proc` 也无需 `libc`；Windows 无 `peer_cred`，准入见「Windows」节），不匹配就 drop 连接并 warn、循环等下一个。**校验是 fail-open**：`admits` 在 peer 或 owner 任一未知时放行，退回 socket 0600 + 目录 0700 那道防线——一个探测不到属主的环境不该让整台机器的 pm3 停摆。拦截 MUST 发生在 `Listener::accept` 里（协议之前），MUST NOT 做成 axum 中间件：那样非法 peer 已经能塞进一个请求体
- 请求体上限走 `pm3.request_body_limit_bytes` + `DefaultBodyLimit`（超限回 413）：单 actor 循环下一个巨大 body 就是一条队头阻塞路径，而 axum 默认的 2 MB 对「一串服务名」来说过宽

### 环境变量与凭据

- 服务的环境变量**只**来自 `cfg_dir/<name>.env`；`<name>.yaml` 的 `env` 字段已删除，残留的 `env:` MUST 报错而非静默忽略（`AppEntry` 没有 `deny_unknown_fields`，不显式拒绝就是无声吞掉）——两个拒绝点见 `adapters/CLAUDE.md`
- **读取点只有 `SpecSource::resolve_service` 一处**：`SpecResolver::prepare`（CLI start）与 `YamlDumpStore::rejoin`（daemon 换代逐条读盘）都经过它。只在 `prepare` 里读会让换代后所有服务 env 变空 ⇒ env 进指纹 ⇒ 全部服务被判 `Change::Launch` 而 evict+respawn
- `.env` **缺失 MUST 是 `Ok(空)`**，只有「存在但解析失败」才 `Err`：`rejoin` 拿到 `Err` 就没有 spec、造不出 `ProcessRecord`，那条记录进不了表
- **进不了表的记录 MUST 被 evict，MUST NOT 静默丢弃**：`DumpStore::load` 返回 `DumpContents { records, stranded }`，`resolve_service` 失败的那条以 `StrandedProcess { name, pid, token }` 进 `stranded`，`resurrect` 开头先 `sweep_stranded`（复用 `surviving_pid` 的 token 守卫 + `evict_pid`）。只 return None 的话，手改 `.env` 打错一个字再换代，正在跑的进程就既不 evict 也不监控、pid 还从 dump 擦除 ⇒ 永久孤儿 + 下次 `start` 起出第二份实例
- **`HOME` 由 pm3 注入**（`SpecSource.host_home` → `with_host_home`，`.env` 声明了同名 key 就以声明为准）：子进程环境被 `env_clear()` 清空，不注入的话服务只能在 `script`/`args` 里写死绝对路径。宿主 `$HOME` 由 `layout.rs::host_home()` 读一次注入（纪律见「配置与路径」）。它随 `spec.env` 进指纹，安全的前提是 unit 会导出安装时的 `HOME`（见「装真机与换代」），launchd/systemd/shell 三种上下文取值一致
- `.env` MUST NOT 过 `substitute_env_vars`：那个替换器遇到含 `"` / `\` / 控制字符的值直接报错，还会把随机密码里的 `${...}` 当占位符展开。`.env` 自己只认**一个**变量 `$HOME`/`${HOME}`（展开成注入的 `host_home`），其余 `$` 一律原样——解析器要守住的三条边界见 `adapters/CLAUDE.md`
- 凭据 MUST NOT 出现在任何错误文案里：`EnvFileError` 只带路径、行号、key 名
- pm3 **只读不写** `.env`（CLI 的 `--env` 已移除），所以 `ServiceUndo` / `reconcile` 都不必管它；只有 `forget` 要连带删除
- 给 `AppSpec` 加字段时，**只有 `usecases/src/fingerprint.rs` 的 `render_identity` 会编译失败**（真·全字段 `let AppSpec { .. }` 解构）：`encode_state` 是 `let ProcessRecord { spec: _, runtime }`、`ProcessView` 是字段访问式构造，两者都会静默吞掉新字段。新字段若含凭据性质的内容，MUST 自己确认这两处。`AppSpec` derive 了 `Debug` 但没有 `Serialize`，MUST NOT 用 `?spec` / `{:?}` 打印它
- `pm3 restart` 会**重新读盘**，所以改完 `.env` / `<name>.yaml` 用 restart 就生效；`on_restart` / `on_fire` / `restart_now` MUST NOT 重读（实现边界见 `usecases/CLAUDE.md`）

### 日志字段

面向 AI 排障，所以字段名比文案重要；改日志前先看这里。

- 每条业务日志 MUST 带 `feature` + `action` 两个字段。**MUST NOT 用 `operation`**：6 必备字段里是 `action`，混用会让按 `action` 过滤的查询整段漏掉。`action` 的值用 `snake_case`，MUST NOT 带点（`drain.start` → `drain_start`）
- `feature` 取值收敛在：`lifecycle` `supervisor` `resurrect` `persistence` `api` `client` `server` `service` `unit` `install`
- 每个**外部调用**（`ps` / `kill` / `launchctl` / `systemctl` / UDS 往返）MUST 记 `duration_ms`：`let started = Instant::now();` 起头，日志里 `started.elapsed().as_millis()`
- 级别按「谁看」分：AI/排障走 `debug`（外部调用、中间状态），人/监控走 `info+`（服务起停成败在 `usecases` 的 `start_one` / `request_stop` 里发）
- **CLI 进程 MUST 自己装 subscriber**，否则 `feature` 为 `client`/`service`/`unit` 的日志全部静默丢弃：装在 `open_session` / `open_service_session`（这两处本来就已读过配置），写 **stderr**（daemon 写 stdout 只因为它被重定向进 `pm3.log`，CLI 的 stdout 是给人看的报文）。`log_stuck_undo` 这类**只有日志一条通路**的 warn（服务文件回滚失败）不在退出码也不在 stderr 文案里，CLI 不装就永远没人看到
- spawn 日志 MUST NOT 打 `args` 与 `env`：服务的启动参数可能含运维塞进去的凭据
- 「尽力而为」的收尾 IO 可以 `.ok()`，但**改变外部可见状态的失败 MUST 记 `warn`**：`force_kill` 失败意味着孤儿进程存活，服务文件回滚失败意味着盘上文件与运行中的服务不一致。吞掉错误后仍记「成功」的日志比没有日志更糟

### 配置与路径

- daemon 自己的 `config.yaml` 只能放在 `pm3.home`：`cfg_dir` 由配置本身定义，放不进去
- `ensure_layout` 把 `pm3.home` 与 `cfg_dir` 收紧到 `0700`，但 **chmod 失败只 warn、MUST NOT 向上抛**：`cfg_dir` 可以指向配置管理预建（root 属主）或只读挂载的目录，一 `?` 就让**每条 CLI 命令**（`prepared_session`）和 daemon 启动一起失败，而目录权限本来只是「更安全一点」的加固
- **pm3 调用的每个外部程序都来自配置**，代码里 MUST NOT 再出现第二份路径常量：`pm3.service.{launchctl,systemctl,loginctl,schtasks,taskkill}_path`（发行版差异大：Debian 在 `/usr/bin`、部分发行版在 `/bin`、NixOS 在 `/run/current-system/sw/bin`；后两个只在 Windows 消费，unix 强杀走硬约束的 `/bin/kill`）、`pm3.sandbox.{seatbelt,bwrap}_program`（`bwrap` 走 `search_path` 解析，`sandbox-exec` 是绝对路径故 `search_path` 对它无效）。例外只有 `/bin/ps` 与 `/bin/kill`（身份令牌与进程组信号的硬约束，见「进程与信号」）
- `PM3_HOME` 同时决定**配置发现**与 `pm3.home`：`default_config_path` 先读 `PM3_HOME` 再回退 `~/.pm3`——只让 `config.yaml` 里的 `${PM3_HOME:-~/.pm3}` 认它的话，`export PM3_HOME=/srv/pm3` 后 `pm3 list` 仍去读 `~/.pm3/config.yaml`
- 读 env 的逻辑 MUST 抽成接 `Option<&str>` 参数的纯函数（`default_config_path(pm3_home_env, home_env)`），env 只在 `frameworks/src/layout.rs` 的 `host_home` / `host_pm3_home` 里读一次：Rust 2024 的 `set_var` 是 `unsafe`，测试无法注入进程级 env
- `substitute_env_vars` **不递归展开默认值**：`${PM3_SEARCH_PATH:-${HOME}/.cargo/bin:...}` 里的 `${HOME}` 会原样留在配置里 → 想让 pm3 找到 `~/.cargo/bin` 下的程序，不要改 `search_path`，直接把服务的 `script` 写成 `${HOME}/.cargo/bin/<prog>`（顶层占位符会展开）
- args 里指代「该服务自己的可写工作目录」MUST 用 `${PM3_SERVICE_CWD}`（命令行写裸 `PM3_SERVICE_CWD`，CLI 折叠成带花括号形式），MUST NOT 写 `${HOME}/.pm3/<name>`（那把 pm3 布局烧进了参数）；只在 args 生效，`cwd`/`writable_roots`/`script` 里写它不展开、会被相对路径校验直接拒；`pm3 describe` 显示的是展开后的真实路径，不能拿它当「配置无绝对路径」的证据
- 服务名 MUST 只含 `[A-Za-z0-9._-]` 且不以 `.` 开头、不能被 `parse::<u32>()` 解析（`entities::validate_app_name`）。校验点在 `service_file_of` **内部**（返回 `Result`）而非各调用方：CLI 是先写盘后交 daemon 校验，只在 `path_safe`（stop/restart/delete/describe）拦一道时，`pm3 start --name ../../../.bashrc` 会先把 yaml 写到 `cfg_dir` 之外、`--force` 还会覆写既有文件。`pm3 logs` 的日志路径同理走 `stdout_log` 的校验：
  - 纯数字会被 `AppSelector::parse` 读成 pm_id，`pm3 stop 3` 会误伤 pm_id=3 的**另一个**服务
  - `/` 与 `..` 会随 `service_file_of` 把服务文件写到 `cfg_dir` 之外（CLI 是先写盘后交 daemon 校验，拦不住）
  - 空格等字符会被原样嵌进 HTTP 请求行，`pm3 stop "my app"` 直接把 request-line 切碎（症状：`the daemon answered nothing`），服务能起却停不掉

## 覆盖率 region 纪律

日常验收是 `just lint` → `just cov`（四指标 100%）。**门禁的运行纪律、四类失败自救与残留清理见 `dev_scripts/CLAUDE.md`**；集成/e2e 技法见 `frameworks/CLAUDE.md`。两个前提：`cargo-llvm-cov` 忽略路径含 `tests/` 的文件而 `test_helpers/`、`test_support/` **计入**门禁（helper 里的 `panic!` 就是未覆盖行）；覆盖率按**函数实例化组**统计、组内取 `max`，所以「同一个 if 被两份实例化各走一半」加多少测试都补不满。

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
- 泛型函数里的 `if` 若被两份实例化各走一半（lib 测试只走 true、e2e 只走 false），覆盖率**加多少测试都补不满**（组内取 max，见上一节）→ MUST 把判断抽成**非泛型**纯函数放进 `usecases`，在那里单测两条臂（实例：`Supervisor::stop_all` 的 `if covered.contains(&pid)` → 抽成 `query::unswept_pids(tracked, scheduled)`）。这类判断本就是业务查询，抽出去顺带把分层也修对了
- 不可达的防御分支应**重写消除**，而非加测试掩盖

## clippy 与库的坑

clippy 四组全开、`-D warnings`，这些是反复撞到的：

- 命名：`similar_names`（`launcher`/`launched`、`receiver`/`received`）、`shadow_unrelated`（闭包参数名与外层 `let` 撞名即报）——换个名字即解
- `elidable_lifetime_names`：`fn f<'s>(x: &'s [T]) -> R<'s>` → `fn f(x: &[T]) -> R<'_>`
- `string_slice` 禁掉一切 `&text[n..]`，哪怕 `n` 是 ASCII 常量的 `.len()`：扫描字符串用 `split_once(pat)` 循环，别 `find` + `split_at` + 切片（`strip_prefix` 也别用，它多一条永不可达的 `else` 分支，覆盖率补不上）
- `unnecessary_join`：`.collect::<Vec<_>>().join("")` → `.collect::<String>()`
- `format_push_string` 与 `format_collect` 互相堵死：`push_str(&format!)` 和 `.map(format!).collect::<String>()` 都报，出路是 `fold(format!(init), |mut t, x| { let _ = writeln!(t, ..); t })`，或把闭包体抽成**具名** fn 再 `.map(named).collect()`（`format_collect` 只盯闭包体是 `format!` 的形态，`launchd.rs` 的 `render_argument` 就是这么写的）
- 测试侧三条：同一 `test_support/*.rs` MUST NOT 被两处 `#[path]` 重复挂载（`duplicate_mod`，统一在 `lib.rs` 以 `#[cfg(test)] pub(crate) mod` 挂一次）；test_helper 的请求构造器 MUST NOT 与 handler 同名（`get`/`post`/`delete` 在 `use super::{test_helpers::*, *}` 下二义 → 用 `get_from`/`post_to`/`delete_at`）；只有 `Ok` 分支的 fixture 触发 `unnecessary_wraps` → fixture 返回裸值、调用处再 `Ok(...)`
- 跨 async 边界的回调参数要写 `&(dyn Fn(&str) + Send + Sync)`，否则外层 future 不是 `Send`
- 结构体从「拥有」改成「借用配置」后，返回 `Foo<'static>` 的 fixture 会编译失败 → 用 `LazyLock<Config>` 让引用变 `'static`
- axum 0.8 原生 `impl Listener for tokio::net::UnixListener`（无需 hyper-util）；`tokio::net::unix::SocketAddr` 只 impl Debug 不 impl Display → 日志用 `?addr`
- clap `trailing_var_arg` + `allow_hyphen_values`：pm3 自身选项必须出现在程序名**之前**，否则被当子进程参数
- **Rust 生态没有任何 cron 库支持 OpenBSD 风格的随机 `~`**（croner/cron/cronexpr/jiff-cron/cron_tab 全无，只有 cronexpr 支持 Jenkins 的固定哈希 `H`）→ 自己展开成具体数字再交 croner

## 真机排障工具

- 判「是不是 OOM」用 `/proc/vmstat` 的 `oom_kill`（开机以来内核 + cgroup OOM 累计杀进程数）：为 0 即可彻底排除，比翻 dmesg/journal 可靠
- 抓「谁杀了进程」MUST 用 `sudo systemd-run --unit=X --collect perf record -a -e syscalls:sys_enter_kill -e signal:signal_generate`：直接从用户会话起的 perf 属 `user-1000.slice`，slice 一崩它就陪葬、数据废掉（`data size field is 0`）；输出里行首是发送者、`comm=`/`pid=` 是目标、`grp=1` 表示进程组广播
- 验证 kill 语义用 `strace -e trace=kill /bin/kill -0 -- <target>`：sig 0 只探测不投递，能看到内核实际收到的 pid
- 托管 sshd 的两条（它是「必须 `danger-full-access`」的典型）：`sshd_config` **不做任何环境变量展开**，`${HOME}`/`$HOME` 都当字面目录名、相对路径按 sshd 的 cwd 解析（而 pm3 把 cwd 设成 `<pm3 home>/<name>`）故必失败，只有 `~` 可用（走 `getpwuid`，`env -i` 下也成立）；验收时本机 `ssh -p <port> 127.0.0.1` 报 `Permission denied (publickey)` 是**正常的**，改为比对 `ssh-keygen -lf <host key>.pub` 与 `ssh -v` 报的 `Server host key` 指纹，且 `pgrep -x sshd` 匹配不到它（proctitle 被改写成 `sshd: ... [listener]`）→ 用 `lsof -nP -iTCP:<port> -sTCP:LISTEN`
