@~/.claude/CLAUDE.md

@~/.claude/clean-arch.md

@~/.claude/rust-p0.md

@~/.claude/rust-p1.md

## 命令与工具

- 优先用 @justfile，禁止随手 `cargo`；例外：`just` 的 recipe 都是 workspace 级，单 crate 迭代用 `cargo <cmd> -p <crate> --release --offline`
- 改过 `Cargo.toml` 后 `--locked` 会直接失败，改用 `--offline`
- `just typecheck` / `just test-scripts` 前先 `bun install --frozen-lockfile`
- `.cargo/config.toml` 把 `release` 的 `opt-level` 设成 0（本地迭代用）→ 装到真机前 MUST `CARGO_PROFILE_RELEASE_OPT_LEVEL=3 cargo build -p frameworks --release --locked`
- `pm3 service install` 用 `current_exe()` 渲染 unit → MUST NOT 在仓库目录执行（会把 plist 钉在 `target/release/pm3`，一次 `cargo clean` 就起不来）；先把二进制 `cp` 到最终位置，再用**那个**二进制执行 install
- `service install` 后 `launchctl list` 的 PID 列可能是 `-`（job 已载入但 launchd 未监管、KeepAlive 形同失效）→ `pm3 kill` 停掉自启的实例，再 `launchctl kickstart gui/$(id -u)/<label>` 交回 launchd
- `just install` 装到真机（`dev_scripts/install.ts`）：opt-level 3 构建 → 备份 → 原子换二进制 → uninstall+kill+等退出 → `install --force` → 等 launchd 真接管 → 比对前后 pid；备份清单从 `service install --dry-run` 的 `write <path>` 行派生，不重复 pm3 的路径推导
- 任何 pm3 CLI 命令都会经 `ensure_daemon_running` 自动拉起一个**非 launchd 托管**的 daemon：它扛不住 `launchctl unload`，且会抢赢 socket 竞争让 launchd 那份直接退出（`launchctl list` PID 列变 `-`）→ 换代顺序 MUST 是 `service uninstall` → `pm3 kill` → 等 `pgrep -f "<bin> daemon"` 归零 → `service install --force`；install 后 MUST 等「launchd 报的 pid == `pm3.pid` 内容」再跑任何 CLI 命令，否则又会拉起竞争者
- 手工验证要另建 pm3 home：scratchpad 路径太长，unix socket 会撞 macOS `SUN_LEN`（>104 字节，报 `path must be shorter than SUN_LEN`）→ 用 `mktemp -d`

## 覆盖率门禁（`just cov`，四指标 100%）

- `cargo-llvm-cov` 忽略路径含 `tests/` 的文件；`test_helpers/` 与 `test_support/` **计入**门禁，helper 里的 `panic!` 会变成未覆盖行
- 改动令行号位移后必须 `just cov --fresh`，否则残留旧实例化产生幽灵 `FNDA:0`
- 全零自救：症状是所有文件 0%、`FNDA:0` 上千条 —— 二进制与 profraw 哈希错位（非 fresh 与手动 `cargo llvm-cov report` 交叉跑会触发）；重跑 `just cov --fresh` 且中途不插任何其他 cargo 命令
- 每个 `?` 的 Err 分支是独立 region，各需一条失败路径测试；`.expect()` / `.unwrap_or(<常量>)` / `.unwrap_or_default()` 不产生本文件 region，「已证不可达」处用 `.expect()` 优于 `map_err` + `?`
- 每个 thiserror variant 都要构造 + `.to_string()` 断言一次，否则该 `Display::fmt` match arm 的 region 不计入覆盖
- tail-return（`f().await` 直接作返回值）不产生 Err region，改成 `let x = f().await?;` 就新增一条；收尾处可用 `f().await.map(|x| ...)`（Err 直传不产生 region，closure 是独立 fn 随 Ok 路径覆盖）；真的失败路径则注入依赖让单测能打
- 定位缺口：`cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'` 找文件；再 `--show-missing-lines`，若它无输出而 summary 有缺口：lines 也缺 → 缺口在 bin 副本（lib+bin 双编译，region 按实例化计数，补 e2e 走真实 binary 或让分支只存在于一处）；lines 100% → 缺的是 `?`/短路的纯 region，重点怀疑新加的 `?`
- `frameworks` 里「只经真实 binary 驱动」的函数 MUST NOT 再补 lib 侧单测：lib 测试会新增一个实例化，函数其余 region 在该实例化里永不可达 → 门禁挂（症状：加测试反而多出 missed region，`--show-missing-lines` 无输出而 lines/branches 均 100%）。修法是删掉 lib 单测、失败路径也走 e2e（如 `pm3 --config /nonexistent kill`）。注意 `main.rs` 只调 `frameworks::cli`、无重复 mod 编译，llvm-cov 仍按「lib test 二进制 + pm3 bin」算两份实例化
- `just cov` 失败但一行文件明细都没打 = 缺的是 **region**（lcov 不含 region 数据，`findFilesBelowFullCoverage` 自然无输出）→ 用上一条的 `--summary-only` 定位，且 MUST 紧跟在一次 `just cov --fresh` 之后跑，查完再回到 `--fresh`
- frameworks 新增的错误分支要走真实 binary：e2e 里 `UnixListener::bind(socket)` 起个假 daemon 回 `200` + 非 JSON body，即可驱动 CLI 的解码失败路径（`frameworks/tests/stale_socket.rs`）；这类假服务器 MUST 用 `while let Ok(..) = accept()` 且 MUST NOT `join()`（只回固定条数会把测试挂住）
- `tracing::debug!(field = <表达式>)` 的表达式只在 subscriber 启用时求值，测试无 subscriber → 该行 region 不覆盖；MUST 先 `let x = <表达式>;` 再 `tracing::debug!(x)`
- `if cond { ... }` 块尾的 `}` 会生成一条独立 region，只有「进入块又走完」才算命中；若该路径不可达就改写成 `if !cond { return ... }` 的早返形式
- 轮询循环的 fall-through `}` 不产生计数（`for _ in 0..=n` 尤甚）→ 让函数返回值并在循环后以 `true` 收尾（`while cond { if 超预算 { return false } ... } true`），fall-through 才有可命中的 region
- `tokio::select!` 展开出的不可达 region 无法覆盖；用一个 forward task 把两个 channel 汇成一个，主循环只 `recv()` 一个 queue
- 泛型/`impl Trait` 参数会按实例化各算一份 region：把 `shutdown: impl Future` 改成 `Pin<Box<dyn Future + Send>>` 可把实例化收敛为一份
- `?` 的 Err region 可达性取决于调用顺序：`canonical_config_path` 排在 `load_and_parse_config` **之后**时其 Err 分支永不可达（文件已读成功 → canonicalize 必成功），把「路径解析」提到「读文件」之前才能覆盖
- 不可注入的系统读取（`std::env::current_exe()`）不要在函数体里直接 `?`；把 `io::Result<T>` 塞进注入的 context，用 `.as_ref().map_err(...)?` 消费，测试才能构造 Err 命中该 region
- 不可达的防御分支应**重写消除**，而非加测试掩盖
- 只有 trait 声明的文件进不了 lcov 会触发「生产文件缺失」→ 不写 blanket impl，让实现方显式 `impl Trait for X {}`

## Rust / clippy

- 同一 `test_support/*.rs` 文件 MUST NOT 被两处 `#[path]` 重复挂载（clippy `duplicate_mod`）；统一在 `lib.rs` 以 `#[cfg(test)] pub(crate) mod` 挂载一次
- test_helper 的请求构造器 MUST NOT 与 handler 同名（`get`/`post`/`delete` 在 `use super::{test_helpers::*, *}` 下二义），用 `get_from`/`post_to`/`delete_at`
- 只有 `Ok` 分支的测试 fixture 会触发 clippy `unnecessary_wraps`：fixture 返回裸值，调用处再 `Ok(...)`
- clippy 会报 `similar_names`（`launcher` 与 `launched`、`receiver` 与 `received`）、`significant_drop_tightening`、`shadow_unrelated`
- 跨 async 边界的回调参数要写 `&(dyn Fn(&str) + Send + Sync)`，否则外层 future 不是 `Send`
- `.collect::<Vec<_>>().join("")` 触发 clippy `unnecessary_join` → 改 `.collect::<String>()`
- `serde_yaml2::to_string` 输出人不可读（键带引号、缩进古怪）：要人可读可改的 yaml MUST 手写渲染器，用「encode → parse 回来相等」的 round-trip 测试兜底
- `main() -> Result<()>` 用 **Debug** 打印错误（`Error: SvcConflict {..}`）：CLI MUST 改 `main() -> ExitCode` + 显式 `eprintln!("{error}")`
- clippy `format_push_string` 与 `format_collect` 互相堵死：`push_str(&format!)` 和 `.map(format!).collect::<String>()` 都报，唯一出路是 `fold(format!(init), |mut t, x| { let _ = writeln!(t, ..); t })`
- `elidable_lifetime_names`：`fn f<'s>(x: &'s [T]) -> R<'s>` → `fn f(x: &[T]) -> R<'_>`
- `shadow_unrelated`：闭包参数名与外层 `let` 撞名即报，换个名字
- 结构体从「拥有」改成「借用配置」后，返回 `Foo<'static>` 的 fixture 会编译失败 → 用 `LazyLock<Config>` 让引用变 `'static`
- clap `trailing_var_arg` + `allow_hyphen_values`：pm3 自身选项必须出现在程序名**之前**，否则被当子进程参数
- 交互询问（confirm prompt）的可测模式：循环签名接 `confirm: &mut (dyn FnMut(&str) -> bool + Send)`，生产传一个「每次调用才锁 stdin/stdout」的 fn（`StdinLock` 非 Send，MUST NOT 跨 `.await` 持有），测试传脚本化闭包；MUST NOT 在单测里碰真 stdin（nextest 下 stdin 是 null → 立即 EOF，且无法注入答案）

## 运行时行为

- `serde_yaml2` 把空 `BTreeMap`/`Vec` 序列化成 `~`，再反序列化成 map 会失败 → 集合字段一律 `#[serde(default, skip_serializing_if = ...)]`
- macOS 上 `sandbox-exec` 的 `subpath` 只认真实路径，`/var/...` 这类符号链接不匹配 → spawn 前必须 canonicalize `cwd` 与 `writable_roots`
- `materialise_workspace` 里展开 `${PM3_SVC_CWD}` MUST 排在 `spec.cwd = real_path(...)` 之后：提前替换会把未 canonicalize 的 cwd 写进 args，正好复现上一条陷阱（回归测试：`adapters/src/tests/workspace_tests.rs::a_placeholder_expands_to_the_real_path_not_the_symlink` 与 `frameworks/tests/sandbox_isolation.rs::a_confined_app_can_write_through_the_cwd_placeholder`）
- 新增 `${...}` 占位符 MUST 在 `substitute_env_vars` 里登记为保留名（`SVC_CWD_NAME` 那个分支），否则加载 cfg 文件时因「变量未设置且无默认值」直接报 `EnvVarNotSet`；保留名不支持 `:-` 默认值
- `TokioProcessLauncher::wait` 会先把 `Child` 从 map 里 remove 再 await，所以「是否存活」必须另用一个 `live: HashSet<u32>` 跟踪（spawn 时插入、wait 返回后删除）
- 服务身份指纹 MUST NOT 含任何宿主环境派生值：`SandboxPolicy` 分 `writable_roots`（运维声明，进指纹）与 `derived_roots`（pm3 从 cwd/logs_dir/`$TMPDIR` 推导，不进指纹），沙箱授予 `granted_roots()` 并集；`render_identity(&AppSpec)` 渲染声明而非包装后的 argv。踩过的坑：launchd 起的 daemon 有 `TMPDIR`、shell 起的没有 → 每次换代都误判 respawn
- `clear_runtime_files` MUST 先删 `pm3.pid` 再删 socket：`pm3 kill` 与 e2e 都以「socket 消失」判定 daemon 收尾完成，反序会让「socket 已没、pid 文件还在」被观测到（症状：`signal_semantics` e2e 约 25% 概率挂在 pid 文件断言）
- 停止/强杀 MUST 先对进程组发信号（`/bin/kill -<SIG> -- -<pid>`）、失败再退回单 pid：spawn 时 `process_group(0)` 让子进程自成组，只杀单 pid 会漏掉它 fork 的孙进程；adopt 来的进程可能不是组长，故回退分支必须保留
- `Stopping` 不是「已停止」：判「pm3 是否还持有进程」用 `ProcessStatus::is_settled()`（仅 Stopped|Errored），用 `!is_running()` 会让重复 `stop` 清空 pid、让 `restart` 再 spawn 一个同名实例
- 熔断判定用 `unstable_restarts >= max_restarts`（对齐 pm2 `God.js`），MUST NOT 改回 `>`
- daemon↔CLI 是 JSON envelope `ReplyDto { report, service, already_running }`：新增命令走 `ask_report`（只要文案）或 `ask`（要结构化字段）；MUST NOT 靠 `.contains(渲染文本)` 反解业务状态
- start 被 daemon 拒绝时 MUST 回滚已写的 `cfg_dir/<name>.yaml`（`svc::SvcUndo` 记前态：原本不存在→删、原本存在→写回）；写盘 MUST NOT 挪到 `ask` 之后——daemon 落 `dump.yaml` 时服务文件必须已在
- 身份指纹 MUST 在 `start_one` spawn 成功那一刻采集：shutdown 时算会把「磁盘上的新哈希」当成旧进程的，重启后误判未变更 → 接管到跑着旧二进制的进程
- 防 pid 复用的身份令牌固定用 `LC_ALL=C ps -ww -o lstart= -p <pid>`（管道下不截断、`LC_ALL=C` 消 locale 漂移）；MUST NOT 换 `etime`（时长需容差）或加 `command=`（`spawn()` 返回时可能尚未 exec，拿到的是旧 argv）
- `resurrect` 判定 respawn 且旧进程仍存活（token 已匹配）时 MUST 先 `terminate` 掉它，否则孤儿与新实例重复运行（症状：`just cov` 跑完残留 `pm3 __sleep`）
- SIGTERM 只落盘退出、不停服务，彻底停机只有 `pm3 kill --with-services` → e2e 收尾 helper MUST 无条件「先 `pm3 list` 拉起 daemon 接管、再 `kill --with-services`」；写成「socket 不存在就 return」会漏掉幸存子进程
- `sh -c "trap '' TERM; sleep 30"` 在被 pm3 spawn 时并不能可靠忽略 SIGTERM（手工 shell 与 python spawn 都能，pm3 路径不能，原因未查明）→ 不要用它当「顽固进程」测试靶子；要覆盖强杀路径就直接调 `on_force_kill`，或先用假的 `on_exit` 让表以为进程已退出
- 集成测试断言「依赖先启动」不能看应用自己写的文件（并发写有竞态），要把 `log_level` 调成 debug 后从 `pm3.log` 里读 `"action":"spawn"` 的顺序
- 测「调用外部服务管理器」（`launchctl`/`systemctl`/`loginctl`）用临时目录里的 `#!/bin/sh` 脚本 + `set_permissions(0o755)` 当替身，可同时控制 stdout 与退出码；真实二进制只用 `/usr/bin/true`、`/usr/bin/false`、`/nonexistent/...`，**绝不**在测试里调真的 `launchctl`/`systemctl`
- 服务 unit 文件位置由 OS 约定在 adapters 里派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），不进配置——单个配置项无法同时对两个平台正确；`$HOME` 由 frameworks 注入，测试传 tempdir 就不会碰真实 `~`
- 断言「子进程环境已清空」MUST 探 `$HOME` 不能探 `$PATH`：`/bin/sh` 在 PATH 缺失时会自己合成一个默认值
- CLI 全局默认值 MUST NOT 在 `execute()` 里现算：e2e 的假进程是 `pm3 __sleep`，子进程环境被 `env_clear()` 清空后没有 `HOME`，「所有子命令都先解析配置路径」会让 sleeper 一启动就退出（症状是 e2e 里 app 显示 `stopped ↺1`）→ 交给 clap `default_value_t` 在构建期算，不读配置的命令自然不受影响
- fixture 里的 `create_dir_all` 会把「测试想要它缺失」的父目录造出来（`save_reports_a_missing_parent_directory` 一度失效）→ 造错误路径的 store/source fixture 必须接一个独立 root，别从被测路径 `parent()` 反推
- e2e 会泄漏 daemon 与子进程（tempdir 已删、进程仍在）：排查真机状态前先 `pgrep -f 'pm3 daemon --config /var/folders'` 与 `pgrep -f 'pm3 __sleep'` 各清一遍，否则 `pgrep`/端口结果会误导；子进程自 `process_group(0)` 起不再随测试进程组被连带清理
- nextest 中断（flake 触发取消剩余测试）会让 `TempDir` 的 Drop 跑不到，在 `$TMPDIR` 留下 e2e fixture 目录（`config.yaml` + `home/{logs,svc,pm3.sock}`）：清理用 `rg -l --hidden 'pm3-e2e-never-installed|pm3-fixture' "$TMPDIR" -g config.yaml` 定位——`rg` 默认跳过隐藏目录而这些正是 `.tmp*`，漏 `--hidden` 会得到假阴性；按 label 指纹而非目录名匹配，才不会误删真机配置
- 文件 IO 错误分支稳定触发：`create_dir_all` 失败 → 目标预置为文件；`rename` 失败 → 目标预置为 non-empty 目录；`remove_file` 失败 → path 是目录；`write` 失败 → 父目录 ENOENT

## 配置与路径约定

- `pm3.home`（`~/.pm3`）放运行时状态（socket/pid/logs/各服务工作目录）+ daemon 自己的 `config.yaml`（`service install` 落盘的那份，unit 的 `--config` 就指它；`cfg_dir` 由配置本身定义，放不进去）；`pm3.cfg_dir`（`~/.config/pm3`）只放每服务一份 `<name>.yaml`
- `dump.yaml` 只存 `services[].runtime`，启动参数全在 `cfg_dir/<name>.yaml`；`YamlDumpStore::load()` 经 `SpecSource` 把两者缝起来，服务文件缺失/损坏只 `warn` 跳过该条
- 服务配置里 MUST NOT 出现绝对路径：`$HOME/` 前缀折成 `${HOME}/`（加载时由 `substitute_env_vars` 展开）、`cwd` 不写（daemon 用 `<pm3.home>/<name>` 推导并建目录）、`script` 存裸名
- 写 `~/.pm3/config.yaml` 与 `cfg_dir/<name>.yaml` 共用 `svc::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖；`service uninstall` 不删配置
- args 里指代「该服务自己的可写工作目录」MUST 用 `${PM3_SVC_CWD}`（命令行写裸 `PM3_SVC_CWD`，CLI 折叠成带花括号形式），MUST NOT 写 `${HOME}/.pm3/<name>`（那把 pm3 布局烧进了参数）；只在 args 生效，`cwd`/`writable_roots`/`script` 里写它不展开、会被相对路径校验直接拒；`pm3 describe` 显示的是展开后的真实路径，不能拿它当「配置无绝对路径」的证据
- `pm3.search_path` 是单一来源：既写进 launchd/systemd unit 的 PATH，也是 daemon 解析 app 程序名的搜索路径；CLI 早期校验必须用它而非 `std::env::var("PATH")`
- 子进程环境默认为空（`tokio_launcher` 有 `env_clear()`），所以 spawn 前必须已解析出绝对路径
- 服务名 MUST NOT 能被 `parse::<u32>()` 解析（`validate_spec` 拒绝）：`AppSelector::parse` 把纯数字读成 pm_id，否则 `pm3 stop 3` 会误伤 pm_id=3 的**另一个**服务
- 给 `SandboxPolicy` 加字段会波及 ~13 处字面量（四层的 test_helpers/test_support）→ 加完先 `cargo build --workspace` 靠 E0063 逐个补齐
- dev_scripts TS：`Bun.env.X` 触发 TS4111 → 写 `Bun.env["X"]`；`Bun.spawn` 不收 `readonly string[]` → 传 `[...command]`
- 给 `Pm3Config` 加字段要同步 6 处：根 `config.yaml`、`adapters/test_support/config_sections.rs`、`adapters/src/test_helpers/config_schema_test_helpers.rs`、`frameworks/test_support/config_fixtures.rs`、`frameworks/tests/common/mod.rs`、校验函数与 `every_error_variant` 表

## 分层

- `frameworks` 不得依赖 `usecases`/`entities`（arch_tests 强制）：内层类型一律从 `adapters` 的具名再导出取
