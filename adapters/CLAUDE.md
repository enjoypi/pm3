# adapters — 双向格式转换与 Port 实现

Controller / Presenter / Gateway / DTO 全在这层。不放业务规则判断，不放用例编排。

## 文件地图

| 目录 | 内容 |
|---|---|
| `config/` | `Pm3Config` schema、loader、`substitute_env_vars`；`app.rs` 是 `pm3 config check/show` |
| `apps_file/` | 用户 apps 文件与单体服务文件的解析、`SpecSource`、`roots`；`env_file.rs` 是 `<name>.env`（环境变量 sidecar）的解析与加载 |
| `http/` | daemon 侧 controller / routes / DTO（`ReplyDto` + `ProcessViewDto`） |
| `logs/` | `pm3 logs` 的 `tail_lines` / `read_tail` / `LogFollower`（`-f` 跟随）；`rotate.rs` 是 copytruncate 写侧切割（O_APPEND 下截断无稀疏洞，保留 1 代 `<name>-*.log.1`） |
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
- `<name>.env` 只被 `load_env_file` 读，永远不被 pm3 写：解析是纯函数 `parse_env_file(path, home, text)`（首个 `=` 切分、`#` 注释、成对引号剥一层、重复 key 报错、双引号与裸值里展开 `$HOME`）。错误文案只带路径/行号/key，MUST NOT 带值。`HOME` 由 `with_host_home` 从注入的 `SpecSource.host_home` 补进去，声明值优先
- `$HOME` 展开 MUST 守住三条边界：`$HOMEBREW_PREFIX` 不能被吃掉（`continues_a_name` 判词尾）、单引号值整体不展开（密码里真写了 `$HOME` 的逃生口）、`host_home` 为 `None` 时不展开
- 加载侧读完 `chmod 0600` MUST 先用 `symlink_metadata` 排除软链（失败只 warn）：`set_permissions` 跟随软链，把 `<name>.env` 链到 `/etc/creds/x.env` 这类共享凭据时会改到**别人的**文件上（症状：另一个消费者突然 EACCES，而 pm3 这边只有一条 debug 日志）
- `env:` 出现在 yaml 里的两个拒绝点：`check_declared_names`（apps 文件走 `split_apps_file`，不 resolve，必须在 parse 期拦）与 `resolve_entry`（服务文件 + 兜底），共用 `reject_env`，报 `AppsFileError::EnvInYaml`
- `SpecSource::resolve_service` 用专属 `parse_service_file` 解析**单体格式**（顶层直接 `name:`/`script:`/…，不包 `apps:` 数组）并按文件名核对 `name`；`apps:` 数组只出现在用户手写的 apps 文件（`pm3 start apps.yaml`）

### 子进程不随父死

三处平台配置必须同时成立，缺一个都会在 daemon 换代时连带杀掉服务：launchd `AbandonProcessGroup`、systemd `KillMode=process`、`bwrap` **不加** `--die-with-parent`。

### 沙箱与路径

- macOS `sandbox-exec` 的 `subpath` 只认真实路径，`/var/...` 这类符号链接不匹配 → spawn 前必须 canonicalize `cwd` 与 `writable_roots`
- 后端程序名从 `SandboxProgramSet` 取（`SandboxBackend::resolve(&programs, search_path)`），`SandboxBackend` 本身只是 `Seatbelt`/`Bwrap` 两个标签、不再持有路径常量；`detect_host_backend` 在 `frameworks` 侧由 `SandboxProgramSet::from_config(&config.sandbox)` 喂入
- `materialise_workspace` MUST NOT 改写 `spec.sandbox.writable_roots`（指纹纪律见根「身份指纹与接管」）：canonicalize 的结果**追加**进 `derived_roots`，与声明值相同时不重复追加
- `materialise_workspace` 里展开 `${PM3_SERVICE_CWD}` MUST 排在 `spec.cwd = real_path(...)` **之后**；提前替换会把未 canonicalize 的 cwd 写进 args，正好复现上面那个陷阱
  回归测试：`src/tests/workspace_tests.rs::a_placeholder_expands_to_the_real_path_not_the_symlink` 与 `frameworks/tests/sandbox_isolation.rs::a_confined_app_can_write_through_the_cwd_placeholder`

#### seatbelt（macOS）

四类路径的语义与「hidden 嵌套方向」的安全约束在根 `CLAUDE.md`「沙箱」，这里只记 SBPL 写法。

- 路径一律走 `-D KEY=值` 参数 + `(param "KEY")`，profile 文本里不出现任何用户路径：这消掉了 SBPL 注入面
- **只认真实路径**：`/etc`、`/var/run` 都是 symlink（→ `/private/etc`、`/private/var/run`），`subpath`/`literal` 写 `/etc/...`、`/var/run/...` 一律不匹配 ⇒ 路径 MUST 写 `/private/...` 形态（mDNSResponder 与 resolv.conf 都栽在这）
- **每条 `subpath` 授权 MUST 配一条 `(allow file-read-metadata (path-ancestors (param ...)))`**（`rules()` 里两行一起写）：用**绝对路径** open granted root 内的文件会逐级 stat 祖先目录，祖先没有 metadata 权限就 EPERM（症状：sqlite 报 `unable to open database file`，而文件属主与权限全正常）。只 chdir 后用相对路径写 cwd 的服务不暴露此坑，所以它能潜伏很久；`read: full` 也不豁免——hidden root 的 carveout 会把落在 `pm3.home` 下的 cwd 祖先一起挡掉
- `network: true` 写 `(allow network-outbound (remote ip))`：裸 `(allow network-outbound)` 连 unix socket 一起放行，等于把 `pm3.sock` 交给服务。Linux 侧不靠这条——实测 `--ro-bind / /` 下 connect 直接 EACCES（只读挂载没有写权限），`--tmpfs` 遮盖后是 ENOENT
- **`network: true` 要能解析域名/读系统 DNS，必须放行两条独立链路**（漏任一：curl/cloudflared 报 `Could not resolve host`，或 mihomo 退回内置 `8.8.8.8` 解析不到内网域名 → 内网服务走 DIRECT 全挂）：
  - `(allow network-outbound (literal "/private/var/run/mDNSResponder"))` + mach-lookup `com.apple.mDNSResponder`/`mDNSResponderHelper`——getaddrinfo 经 mDNSResponder（unix socket + mach port）解析
  - `(allow file-read* file-test-existence (literal "/private/var/run/resolv.conf"))`——Go 解析器读 resolv.conf 拿系统 DNS（内网 nameserver）。与 mDNSResponder 是**两条路**：curl 走前者通时 mihomo（读文件）仍可能卡后者
  - 验证法：`sandbox-exec -p "$(cat <拼接的profile>)"` 里 `cat /var/run/resolv.conf` 应读出内网 nameserver、`curl https://内网域名` 应 302、`nc -z -U ~/.pm3/pm3.sock` 仍 exit 1（pm3.sock 隔离不得削弱）
- **用 Metal/GPU 的服务（llama.cpp 之类）**：base policy 放行 iokit `AGXDeviceUserClient`/`IOGPUDeviceUserClient`/`IOSurfaceRootUserClient` + mach-lookup `com.apple.MTLCompilerService` + sysctl `hw.cpusubfamily`，minimal read 放 `/System/Library/Extensions`（GPU driver bundle）、`/System/Library/CoreServices` 与 `path-ancestors "/System/Library"`。缺 iokit 报 `ggml_metal_init: picking default device: (null)`，缺编译服务报 `does not have a precompiled Metal library` 后仍 `failed to create llama_context`。实测**不需要** `com.apple.windowserver.active`，别为省事放它
- **递归 `fs.watch` 的服务（Node/前端 dev server）MUST 有 mach-lookup `com.apple.FSEvents`**：libuv 在 macOS 走 FSEvents，被拒后**静默回退**到 kqueue 逐目录开 fd，扫过 `node_modules` 就 `EMFILE: too many open files, watch` 并崩掉整个进程。症状极具误导性——报的是 fd 耗尽，抬 `ulimit -n` 只把爆点推后（实测 65536 仍炸），而沙箱外同一份代码完全正常。**只 deny 一次**（初始化失败即全程回退），在 `log stream` 里极易被 GPU/日志类噪声淹没
- 服务报 `Operation not permitted` 时先看它调的外部程序是不是 perl 脚本：macOS 无 `sha256sum`，`shasum` 是 `/usr/bin/perl` 脚本，minimal read 下 `/System/Library/Perl/*/CORE/libperl.dylib` 不可读 → dyld 直接失败，而调用方常把它当「命令输出为空」处理（症状：`sha256 mismatch: expected <值>, got `，末尾空）
- 查「沙箱到底拒了什么」用 `log stream --style compact --predicate 'eventMessage CONTAINS "deny"'`（另起后台再跑被测程序）：SBPL 的 `(trace "<file>")` 在现代 macOS 不产出文件，别在它上面浪费时间

#### bwrap（Linux）

- **MUST NOT 加 `--new-session`**（codex 加了，pm3 不能）：setsid 会让服务脱离 bwrap 的进程组，`kill -TERM -<pgid>` 打不到它，优雅停止退化成「杀 bwrap → pid namespace 塌掉 → 内核 SIGKILL」。TIOCSTI 面靠内核 `dev.tty.legacy_tiocsti=0`（Linux 6.2+ 默认）与「stdin 是 `Stdio::null()`」兜底
- namespace 里 cgroup 那条 MUST 写 `--unshare-cgroup-try` 而非 `--unshare-cgroup`：cgroup namespace 要内核 4.6+，硬形式在更老的内核上让 bwrap 直接退出 ⇒ 服务起不来。`user`/`pid`/`ipc`/`uts` 用硬形式（这四个到处都有）
- 挂载顺序是安全语义：`--tmpfs <hidden>` → `--bind <granted>`（最浅的先）→ **再** `--tmpfs` 那些落在 granted 之下的 hidden（`nested_in`）
- MUST NOT 加 `--die-with-parent`（子进程不随 daemon 换代而死，见上「子进程不随父死」）

### 进程

- `TokioProcessLauncher::wait` 会先把 `Child` 从 map 里 remove 再 await，所以「是否存活」必须另用一个 `live: HashSet<u32>` 跟踪（spawn 时插入、wait 返回后删除）
- adopt 来的进程不是子进程（`adopt` 只插 `live`、不插 `children`），只能轮询 `ps`；而 daemon 换代后**每个**未变更的服务都走这条路 → 探活 MUST 经共享的 `AdoptedWatch`：一个 poller task 每 tick 发一条 `ps -ww -o pid=,lstart= -p <csv>` 覆盖全部被监视 pid，各等待者 await 自己的 oneshot。每 pid 一个 task 各自 fork `ps` 时，20 个服务就是 20 个子进程/秒。`PsProcessProbe::identity` 复用 `identities(&[pid])`，token 仍是 `lstart` 文本（跨版本可比）
- `kill_signaler`（进程组信号）与 `ps_probe`（`ps -o lstart=` 身份令牌）的取值方式是硬约束，见根 `CLAUDE.md` 的「进程与信号」「身份指纹与接管」
- **三条 `ps` 各司其职，MUST NOT 合并**：`BATCH_FORMAT`（`pid=,lstart=`，身份令牌，`parse_report` 按第一个空格切、余下整段当令牌）、`resident_memory_kib`（`pid=,rss=`，内存熔断 tick）、`resource_usage`（`pid=,rss=,pcpu=`，`list`/`describe` 的资源列，请求路径现采不进 tick）。往第一条加列即让内存波动被误判成 pid 复用（后果见根「内存熔断」）
- `AdoptedWatch` 不能复用给自己 spawn 的子进程：`wait_for_exit` 先判 `is_child`，子进程走 `Child::wait`，所以那条轮询常态下是空的；它的 cadence 会退避到 `daemon_poll_max_interval_ms`

### 调度

- `random_expand.rs` MUST 在**每次 `arm_timer`** 才把 `~`/`a~b`/`a~b/n` 展开成具体数字交 croner；只在加载时展开一次就丢掉了「每次触发重新摇号」这个需求
- croner 的 `find_next_occurrence` 会把入参 `start` 的**亚秒余数原样带进结果**（`10:00:00.400` 问出 `10:55:00.400`）→ 同一个 cron 周期在不同时刻问会得到不同的 `fire_at_ms`，两次 `arm` 一旦跨整秒边界，`Fire` 事件就因 `fire_is_due` 比对不上而被丢弃（症状：cron 测试约 0.5% 概率 flake，`next_fire_ms` 尾巴上多个 `001`）。MUST 在 `next_fire_ms` 里先把入参截到整秒（`after_ms - after_ms % MILLIS_PER_SECOND`）

### 服务管理器

- unit 文件位置由 OS 约定在**本层**派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），**不进配置**——单个配置项无法同时对两个平台正确；`$HOME` 由 `frameworks` 注入，测试传 tempdir 就不会碰真实 `~`。但**管理器二进制的路径进配置**（`ServiceProgramSet::from_config`，见根 `CLAUDE.md`「配置与路径」）：`ServiceProgramSet` 刻意没有 `Default` impl，唯一生产构造点是 `open_service_session`，测试仍可经 `ServiceContext.programs: Option<&_>` 注入替身
- **systemd 与 launchd 的「何时重启」语义靠 `restart_condition` 统一**：`always` → systemd `Restart=always` / launchd `KeepAlive=<true/>`；`on-failure` → systemd `Restart=on-failure` / launchd `KeepAlive=<dict><key>SuccessfulExit</key><false/></dict>`。默认值取 `always`（与 `restart.autorestart: true` 的意图一致；旧版两管理器行为不一致，升级后要旧语义就显式设 `pm3.service.restart_condition: on-failure`）。Windows（schtasks）不参与这张表：Task Scheduler 只有 RestartOnFailure，统一语义由 `.cmd` 包装脚本末尾的恒 `exit /b 1` 实现，`restart_condition` 两个取值在 Windows 上行为相同（见根 `CLAUDE.md`「Windows」节）
- systemd 的转义表只剩 `escape_value` 一份，`quote_token` 委托它加一对引号 → 补转义规则只改 `escape_value`；MUST NOT 再复制一份——漏改一处就会出现「参数被 systemd 二次解析、unit 装得上却起不来」
- `UnitCommand.env` 只有 `user_scoped()` 会填（一条 `XDG_RUNTIME_DIR`，值来自 `UnitProgramSet.runtime_dir`）：`systemctl --user` 在非登录会话里没有它就连不上 user bus，而 launchctl 与 loginctl 走的都不是 user bus，故它们的命令 env 必须为空。新增 systemctl 子命令 MUST 经 `user_scoped`，别自己拼 `--user`
- `install_plan` 接 `LingerState`：`Enabled` 时不生成 `enable-linger` 那步（它走 polkit，非交互会话必失败）。判定在 `runner::linger_state`，launchd 直接回 `Unknown`（不 fork loginctl）；`dry_run` 也会先查一次，所以假 loginctl 替身要按 `$1` 分派（`show-user` 回 `yes`/`no`，`enable-linger` 另算）
- **只有 `systemctl --user` 依赖 `XDG_RUNTIME_DIR`**（`loginctl` 走 system bus，不受影响——排查时别把两者混为一谈）：非登录会话（agent/CI shell）里它为空 → 走 systemctl 的调用失败，报文视 systemd 版本而异（`Failed to connect to bus: No medium found`，或 `... $DBUS_SESSION_BUS_ADDRESS and $XDG_RUNTIME_DIR not defined`）。`frameworks` 的 `host_runtime_dir()` 缺该变量时按 `/run/user/<uid>` 推导（uid 取 `/proc/self` 的属主，macOS 无 `/proc` 故为 `None`，launchd 也不需要），经 `UnitProgramSet.runtime_dir` 只注给 `systemctl --user` 那几条命令 ⇒ `just install` 与 `pm3 service *` 不需要手工 export；`systemctl_show_main_pid`（`pm3 install` 查 MainPID）同样经 `user_scoped` 注入
- `loginctl enable-linger` **不带用户名是合法的**（`enable-linger [USER...]`，省略即作用于调用者）：无授权时它只报 polkit `requires interactive authentication`、**不报缺参数** → 别以为是命令写错了去补用户名。故它在 plan 里是 `TryRun`（失败只 warn 并追加 `skipped: ...`），MUST NOT 改回 `Run`：unit 与 enable 都已生效，整体报 rv=1 会让运维以为没装上。仍看到 `skipped:` 说明 linger 真没开，由有 sudo 的账号补一次，否则用户注销后 user manager 回收会连带停掉 daemon
- 查 linger MUST 传 uid（`loginctl show-user <uid> -p Linger --value`）：**不带用户名时它输出空串且 rv=0**（`show-user` 省略参数指「当前会话的用户」，非登录会话没有会话）→ 按「非零退出才是查不到」判会把「已开 linger」误读成未开，白跑一次必失败的 `enable-linger`。故 `loginctl_show_linger` 在 uid 未知时返回 `None`，判定退回 `Unknown`
- `capture()` MUST 包 `command_timeout_ms` 超时：`systemctl --user` 在无 bus 的非登录会话里、`launchctl load` 在 launchd 繁忙时都可能长时间挂住，没有超时会让 `pm3 service install/uninstall` 无限期卡死。失败走 `ServiceCommandError::Stalled`
- `ServiceStep::Run` 失败即中止整个 plan，`TryRun` 失败只记 warn 并把原因收进 `execute_plan` 返回的 skipped 列表（`install_service`/`uninstall_service` 都追加到报告末尾）→ 「装不上就该报错」的步骤用 `Run`，「装不上也无妨、只影响后续可用性」的用 `TryRun`
- **卸载路径的服务管理器调用一律 `TryRun`**：`launchctl unload` / `systemctl --user disable --now` / `daemon-reload` 用 `Run` 会让「job 未 load」或「无 bus 的非登录会话」把 `Remove{unit_path}` 挡在后面，unit 文件永远删不掉、重跑每次同样失败，而根 `CLAUDE.md` 的换代顺序第一步正是 `service uninstall`。卸载的成败标准是「unit 文件没了」，不是「管理器答应了」
- `execute_plan` 失败时会**回滚本次新建的文件**（`Write` 前 `try_exists` 为 false 的那些）：install 的 `load` 失败不会留下半装状态让 `query_status` 谎报 `installed, not running`；已存在、只是被覆写的文件不回滚（那可能是运维自己的）

### 意图落盘的唯一改点

- 写 `cfg_dir/<name>.yaml` 的两条路径（apps 文件与 `pm3 start --name`）MUST 共用**同一个** `fold_entry`：它把 `script`/`cwd`/`args`/`sandbox.writable_roots` 四处折回 `${HOME}`/`${PM3_SERVICE_CWD}` 并对 roots 去重。出现第二份副本必然分歧，症状是同一份声明编码出两种 yaml、`pm3 start <apps-file>` 被 `reconcile` 拒绝（diff 只差一行重复的 root，或全是 `-"${HOME}/x"` / `+"/Users/me/x"`）。新增含路径的字段只改 `fold_entry` 一处
- 新增 `${...}` 占位符 MUST 在 `substitute_env_vars` 里登记为保留名（`SERVICE_CWD_NAME` 那个分支），否则加载 cfg 文件时因「变量未设置且无默认值」直接报 `EnvVarNotSet`；保留名不支持 `:-` 默认值

### 服务文件读写

- 读服务文件/配置文件 MUST 区分「不存在」与「读失败」（`read_existing` 只把 `NotFound` 当 `None`，其余回 `ServiceError::Read`）：`read_to_string(..).unwrap_or_default()` / `.ok()` 会把非 UTF-8 内容、权限不足、路径已变成目录都当成「文件不存在」⇒ `reconcile` 判 Stale，不带 `--force` 也静默覆写运维的配置；且 `ServiceUndo` 把前态记成 `None` 后，回滚走 `Restore::Remove` —— **删掉**用户原有文件而非还原它
- 空文件不等于文件不存在：内容为空但与新内容不同时 MUST 走 `Conflict`（要 `--force`），不要沿用「`existing.is_empty()` 即 Stale」
- `forget`（删服务文件）失败 MUST 记 warn（`NotFound` 除外）：吞掉后 `pm3 delete` 照样报成功，盘上文件与 daemon 状态从此不一致，下次 `start` 被 `reconcile` 拿旧文件打 diff 拒绝，而原因在任何日志里都没有痕迹

### 日志读侧

- `read_tail` 回扫到 `pm3.log_read_max_bytes` 预算即停，首行可能被切 → `start > 0` 时丢弃缓冲区首个不完整行；`LogFollower.pending` 超限即整段作为一行释放。`LogFollower::resync` 对 truncate 与 rename+recreate 都已兼容（上限的理由见根 `CLAUDE.md`「日志」）

### 序列化

- `serde_yaml2` 把空 `BTreeMap`/`Vec` 序列化成 `~`，再反序列化成 map 会失败 → 集合字段一律 `#[serde(default, skip_serializing_if = ...)]`
- `serde_yaml2::to_string` 输出人不可读（键带引号、缩进古怪）：要人可读可改的 yaml MUST 手写渲染器，用「encode → parse 回来相等」的 round-trip 测试兜底

### DTO

- 给 `ProcessView` 加字段会把 `DaemonReply::Described` 撑过 clippy `large_enum_variant` 阈值 → 在 enum 上加 `#[expect(..., reason = "one reply travels per CLI command")]`，别为过 lint 而 Box（会波及 controller/presenter 全链路）
