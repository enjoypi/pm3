# adapters — 双向格式转换与 Port 实现

Controller / Presenter / Gateway / DTO 全在这层。不放业务规则判断，不放用例编排。

## 文件地图

| 目录 | 内容 |
|---|---|
| `config/` | `Pm3Config` schema、loader、`substitute_env_vars`；`app.rs` 是 `pm3 config check/show` |
| `apps_file/` | 用户 apps 文件与单体服务文件的解析、`SpecSource`、`roots`；`env_file.rs` 是 `<name>.env`（环境变量 sidecar）的解析与加载 |
| `http/` | daemon 侧 controller / routes / DTO（`ReplyDto` + `ProcessViewDto`） |
| `logs/` | `pm3 logs` 的 `tail_lines` / `read_tail` / `LogFollower`（`-f` 跟随）；`rotate.rs` 是 copytruncate 写侧切割 |
| `persistence/` | `dump.yaml` 的 `yaml_store` 与 DTO |
| `presenter/` | `list` 表格、`describe`、reply 文案、`json`（`--json` 输出）；`fields.rs` 是单字段格式化 |
| `process/` | `tokio_launcher` `kill_signaler` `ps_probe`（`resource_usage` 走独立一条 `pid=,rss=,pcpu=`） `ready_probe`（宿主侧就绪探测） `sha256_fingerprinter` `system_clock` `watcher` `timed`（`capture_timed`：外部命令 + 超时的唯一骨架，三态 `CommandOutcome::{Stalled, SpawnFailed, Finished}`，`kill_signaler` / `ps_probe` / `unit::runner` 共用，各自只写自己的日志与错误映射） |
| `sandbox/` | `seatbelt`（macOS，含 `.sbpl`）/ `bwrap`（Linux）/ `wrapper` / `backend` |
| `schedule/` | `cron_scheduler` + `random_expand`（OpenBSD `~` 展开） |
| `unit/` | OS 服务单元：`spec.rs`（`UnitKind` / `unit_dir_of` / `parse_launchd_pid` / `parse_main_pid`）、`launchd` / `systemd` / `schtasks`（Windows Task Scheduler XML + `.cmd` 包装脚本）unit 渲染、`escape.rs`（两个 XML 渲染器共用一份转义表）、`plan`（含 `write_targets`）`actions` `runner`（含 `query_supervised_pid` / `hand_back_to_manager`）`command` |
| `install/` | `pm3 install` 的 Gateway：`layout`（destination/备份根/stamp 纯函数）、`store`（`back_up` 0700/0600、`replace_binary` 的 `.incoming`+rename）、`InstallError` |
| `service/` | pm3 服务文件 Gateway：`store`（读写 / `reconcile` / `ServiceUndo`）、`prepare`（`prepare_inline` / `split_apps_file`） |
| 根文件 | `paths.rs` `program.rs` `startup.rs` `state.rs` `workspace.rs` |

## 本层规则

### 意图与运行态的缝合

- `load_apps_file` / `load_service_file` / `SpecSource::resolve_service` 都是 **async**：它们跑在 daemon 单任务 actor 循环里（`Daemon::start` 每服务一次、`YamlDumpStore::load` 每记录一次），用同步 `std::fs` 会在慢盘/NFS 上冻住整个事件循环。同理 `materialise_workspace` 用 `tokio::fs::canonicalize` 并对已解析过的路径去重（cwd 会同时出现在 `spec.cwd` 与 `derived_roots[0]`）
- `cfg_dir/<name>.yaml` 存**意图**（零绝对路径，`${HOME}` 占位、`script` 存裸名、`cwd` 由 daemon 推导），`dump.yaml` 只存 `services[].runtime`；`SpecSource` 在 daemon 启动时把两者缝起来，服务文件缺失/损坏 MUST NOT 让整个 daemon 起不来——`rejoin` 记 `warn` 后把那条转成 `DumpContents.stranded`（`StrandedProcess { name, pid, token }`）交给 `resurrect` 清扫，MUST NOT 直接 `return None`（详见根 `CLAUDE.md`「环境变量与凭据」）
- `<name>.env` 只被 `load_env_file` 读，永远不被 pm3 写：解析是纯函数 `parse_env_file(path, home, text)`（首个 `=` 切分、`#` 注释、成对引号剥一层、重复 key 报错、双引号与裸值里展开 `$HOME`），加载侧读完 `chmod 0600`（软链跳过、失败只 warn，见根 `CLAUDE.md`「环境变量与凭据」）。错误文案只带路径/行号/key，MUST NOT 带值。`HOME` 由 `with_host_home` 从注入的 `SpecSource.host_home` 补进去，声明值优先
- `SpecSource::resolve_service` 用专属 `parse_service_file` 解析**单体格式**（顶层直接 `name:`/`script:`/…，不包 `apps:` 数组）并按文件名核对 `name`；`apps:` 数组只出现在用户手写的 apps 文件（`pm3 start apps.yaml`）

### 子进程不随父死

三处平台配置必须同时成立，缺一个都会在 daemon 换代时连带杀掉服务：launchd `AbandonProcessGroup`、systemd `KillMode=process`、`bwrap` **不加** `--die-with-parent`。

### 沙箱与路径

- macOS `sandbox-exec` 的 `subpath` 只认真实路径，`/var/...` 这类符号链接不匹配 → spawn 前必须 canonicalize `cwd` 与 `writable_roots`
- 后端程序名从 `SandboxProgramSet` 取（`SandboxBackend::resolve(&programs, search_path)`），`SandboxBackend` 本身只是 `Seatbelt`/`Bwrap` 两个标签、不再持有路径常量；`detect_host_backend` 在 `frameworks` 侧由 `SandboxProgramSet::from_config(&config.sandbox)` 喂入
- `materialise_workspace` MUST NOT 改写 `spec.sandbox.writable_roots`（指纹纪律见根「身份指纹与接管」）：canonicalize 的结果**追加**进 `derived_roots`，与声明值相同时不重复追加
- `materialise_workspace` 里展开 `${PM3_SERVICE_CWD}` MUST 排在 `spec.cwd = real_path(...)` **之后**；提前替换会把未 canonicalize 的 cwd 写进 args，正好复现上面那个陷阱
  回归测试：`src/tests/workspace_tests.rs::a_placeholder_expands_to_the_real_path_not_the_symlink` 与 `frameworks/tests/sandbox_isolation.rs::a_confined_app_can_write_through_the_cwd_placeholder`

### 进程

- `TokioProcessLauncher::wait` 会先把 `Child` 从 map 里 remove 再 await，所以「是否存活」必须另用一个 `live: HashSet<u32>` 跟踪（spawn 时插入、wait 返回后删除）
- adopt 来的进程不是子进程（`adopt` 只插 `live`、不插 `children`），只能轮询 `ps`；而 daemon 换代后**每个**未变更的服务都走这条路 → 探活 MUST 经共享的 `AdoptedWatch`：一个 poller task 每 tick 发一条 `ps -ww -o pid=,lstart= -p <csv>` 覆盖全部被监视 pid，各等待者 await 自己的 oneshot。每 pid 一个 task 各自 fork `ps` 时，20 个服务就是 20 个子进程/秒。`PsProcessProbe::identity` 复用 `identities(&[pid])`，token 仍是 `lstart` 文本（跨版本可比）
- `kill_signaler`（进程组信号）与 `ps_probe`（`ps -o lstart=` 身份令牌）的取值方式是硬约束，见根 `CLAUDE.md` 的「进程与信号」「身份指纹与接管」

### 调度

- `random_expand.rs` MUST 在**每次 `arm_timer`** 才把 `~`/`a~b`/`a~b/n` 展开成具体数字交 croner；只在加载时展开一次就丢掉了「每次触发重新摇号」这个需求
- croner 的 `find_next_occurrence` 会把入参 `start` 的**亚秒余数原样带进结果**（`10:00:00.400` 问出 `10:55:00.400`）→ 同一个 cron 周期在不同时刻问会得到不同的 `fire_at_ms`，两次 `arm` 一旦跨整秒边界，`Fire` 事件就因 `fire_is_due` 比对不上而被丢弃（症状：cron 测试约 0.5% 概率 flake，`next_fire_ms` 尾巴上多个 `001`）。MUST 在 `next_fire_ms` 里先把入参截到整秒（`after_ms - after_ms % MILLIS_PER_SECOND`）

### 服务管理器

- unit 文件位置由 OS 约定在**本层**派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），**不进配置**——单个配置项无法同时对两个平台正确；`$HOME` 由 `frameworks` 注入，测试传 tempdir 就不会碰真实 `~`。但**管理器二进制的路径进配置**（`ServiceProgramSet::from_config`，见根 `CLAUDE.md`「配置与路径」）：`ServiceProgramSet` 刻意没有 `Default` impl，唯一生产构造点是 `open_service_session`，测试仍可经 `ServiceContext.programs: Option<&_>` 注入替身
- **systemd 与 launchd 的「何时重启」语义靠 `restart_condition` 统一**：`always` → systemd `Restart=always` / launchd `KeepAlive=<true/>`；`on-failure` → systemd `Restart=on-failure` / launchd `KeepAlive=<dict><key>SuccessfulExit</key><false/></dict>`。默认值取 `always`（与 `restart.autorestart: true` 的意图一致；旧版两管理器行为不一致，升级后要旧语义就显式设 `pm3.service.restart_condition: on-failure`）。Windows（schtasks）不参与这张表：Task Scheduler 只有 RestartOnFailure，统一语义由 `.cmd` 包装脚本末尾的恒 `exit /b 1` 实现，`restart_condition` 两个取值在 Windows 上行为相同（见根 `CLAUDE.md`「Windows」节）
- systemd 的转义表只剩 `escape_value` 一份，`quote_token` 委托它加一对引号 → 补转义规则只改 `escape_value`；MUST NOT 再复制一份——漏改一处就会出现「参数被 systemd 二次解析、unit 装得上却起不来」
- `UnitCommand.env` 只有 `user_scoped()` 会填（一条 `XDG_RUNTIME_DIR`，值来自 `UnitProgramSet.runtime_dir`）：`systemctl --user` 在非登录会话里没有它就连不上 user bus，而 launchctl 与 loginctl 走的都不是 user bus，故它们的命令 env 必须为空。新增 systemctl 子命令 MUST 经 `user_scoped`，别自己拼 `--user`
- `install_plan` 接 `LingerState`：`Enabled` 时不生成 `enable-linger` 那步（它走 polkit，非交互会话必失败）。判定在 `runner::linger_state`，launchd 直接回 `Unknown`（不 fork loginctl）；`dry_run` 也会先查一次，所以假 loginctl 替身要按 `$1` 分派（`show-user` 回 `yes`/`no`，`enable-linger` 另算）
- `capture()` MUST 包 `command_timeout_ms` 超时：`systemctl --user` 在无 bus 的非登录会话里、`launchctl load` 在 launchd 繁忙时都可能长时间挂住，没有超时会让 `pm3 service install/uninstall` 无限期卡死。失败走 `ServiceCommandError::Stalled`
- `ServiceStep::Run` 失败即中止整个 plan，`TryRun` 失败只记 warn 并把原因收进 `execute_plan` 返回的 skipped 列表（`install_service`/`uninstall_service` 都追加到报告末尾）→ 「装不上就该报错」的步骤用 `Run`，「装不上也无妨、只影响后续可用性」的用 `TryRun`
- **卸载路径的服务管理器调用一律 `TryRun`**：`launchctl unload` / `systemctl --user disable --now` / `daemon-reload` 用 `Run` 会让「job 未 load」或「无 bus 的非登录会话」把 `Remove{unit_path}` 挡在后面，unit 文件永远删不掉、重跑每次同样失败，而根 `CLAUDE.md` 的换代顺序第一步正是 `service uninstall`。卸载的成败标准是「unit 文件没了」，不是「管理器答应了」
- `execute_plan` 失败时会**回滚本次新建的文件**（`Write` 前 `try_exists` 为 false 的那些）：install 的 `load` 失败不会留下半装状态让 `query_status` 谎报 `installed, not running`；已存在、只是被覆写的文件不回滚（那可能是运维自己的）

### 服务文件读写

- 读服务文件/配置文件 MUST 区分「不存在」与「读失败」（`read_existing` 只把 `NotFound` 当 `None`，其余回 `ServiceError::Read`）：`read_to_string(..).unwrap_or_default()` / `.ok()` 会把非 UTF-8 内容、权限不足、路径已变成目录都当成「文件不存在」⇒ `reconcile` 判 Stale，不带 `--force` 也静默覆写运维的配置；且 `ServiceUndo` 把前态记成 `None` 后，回滚走 `Restore::Remove` —— **删掉**用户原有文件而非还原它
- 空文件不等于文件不存在：内容为空但与新内容不同时 MUST 走 `Conflict`（要 `--force`），不要沿用「`existing.is_empty()` 即 Stale」
- `forget`（删服务文件）失败 MUST 记 warn（`NotFound` 除外）：吞掉后 `pm3 delete` 照样报成功，盘上文件与 daemon 状态从此不一致，下次 `start` 被 `reconcile` 拿旧文件打 diff 拒绝，而原因在任何日志里都没有痕迹

### 序列化

- `serde_yaml2` 把空 `BTreeMap`/`Vec` 序列化成 `~`，再反序列化成 map 会失败 → 集合字段一律 `#[serde(default, skip_serializing_if = ...)]`
- `serde_yaml2::to_string` 输出人不可读（键带引号、缩进古怪）：要人可读可改的 yaml MUST 手写渲染器，用「encode → parse 回来相等」的 round-trip 测试兜底

### DTO

- 给 `ProcessView` 加字段会把 `DaemonReply::Described` 撑过 clippy `large_enum_variant` 阈值 → 在 enum 上加 `#[expect(..., reason = "one reply travels per CLI command")]`，别为过 lint 而 Box（会波及 controller/presenter 全链路）
