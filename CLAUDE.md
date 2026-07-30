@~/.claude/CLAUDE.md

@~/.claude/clean-arch.md

@~/.claude/rust-p0.md

@~/.claude/rust-p1.md

## 命令与工具

- 优先用 @justfile，禁止随手 `cargo`；例外：`just` 的 recipe 都是 workspace 级，单 crate 迭代用 `cargo <cmd> -p <crate> --release --offline`
- 改过 `Cargo.toml` 后 `--locked` 会直接失败，改用 `--offline`
- `just typecheck` / `just test-scripts` 前先 `bun install --frozen-lockfile`

## 覆盖率门禁（`just cov`，四指标 100%）

- `cargo-llvm-cov` 忽略路径含 `tests/` 的文件；`test_helpers/` 与 `test_support/` **计入**门禁，helper 里的 `panic!` 会变成未覆盖行
- 改动令行号位移后必须 `just cov --fresh`，否则残留旧实例化产生幽灵 `FNDA:0`
- 全零自救：症状是所有文件 0%、`FNDA:0` 上千条 —— 二进制与 profraw 哈希错位（非 fresh 与手动 `cargo llvm-cov report` 交叉跑会触发）；重跑 `just cov --fresh` 且中途不插任何其他 cargo 命令
- 每个 `?` 的 Err 分支是独立 region，各需一条失败路径测试；`.expect()` / `.unwrap_or(<常量>)` / `.unwrap_or_default()` 不产生本文件 region，「已证不可达」处用 `.expect()` 优于 `map_err` + `?`
- 每个 thiserror variant 都要构造 + `.to_string()` 断言一次，否则该 `Display::fmt` match arm 的 region 不计入覆盖
- tail-return（`f().await` 直接作返回值）不产生 Err region，改成 `let x = f().await?;` 就新增一条；收尾处可用 `f().await.map(|x| ...)`（Err 直传不产生 region，closure 是独立 fn 随 Ok 路径覆盖）；真的失败路径则注入依赖让单测能打
- 定位缺口：`cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'` 找文件；再 `--show-missing-lines`，若它无输出而 summary 有缺口：lines 也缺 → 缺口在 bin 副本（lib+bin 双编译，region 按实例化计数，补 e2e 走真实 binary 或让分支只存在于一处）；lines 100% → 缺的是 `?`/短路的纯 region，重点怀疑新加的 `?`
- `tracing::debug!(field = <表达式>)` 的表达式只在 subscriber 启用时求值，测试无 subscriber → 该行 region 不覆盖；MUST 先 `let x = <表达式>;` 再 `tracing::debug!(x)`
- `if cond { ... }` 块尾的 `}` 会生成一条独立 region，只有「进入块又走完」才算命中；若该路径不可达就改写成 `if !cond { return ... }` 的早返形式
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
- `sh -c "trap '' TERM; sleep 30"` 在被 pm3 spawn 时并不能可靠忽略 SIGTERM（手工 shell 与 python spawn 都能，pm3 路径不能，原因未查明）→ 不要用它当「顽固进程」测试靶子；要覆盖强杀路径就直接调 `on_force_kill`，或先用假的 `on_exit` 让表以为进程已退出
- 集成测试断言「依赖先启动」不能看应用自己写的文件（并发写有竞态），要把 `log_level` 调成 debug 后从 `pm3.log` 里读 `"action":"spawn"` 的顺序
- 测「调用外部服务管理器」（`launchctl`/`systemctl`/`loginctl`）用临时目录里的 `#!/bin/sh` 脚本 + `set_permissions(0o755)` 当替身，可同时控制 stdout 与退出码；真实二进制只用 `/usr/bin/true`、`/usr/bin/false`、`/nonexistent/...`，**绝不**在测试里调真的 `launchctl`/`systemctl`
- 服务 unit 文件位置由 OS 约定在 adapters 里派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），不进配置——单个配置项无法同时对两个平台正确；`$HOME` 由 frameworks 注入，测试传 tempdir 就不会碰真实 `~`
- 断言「子进程环境已清空」MUST 探 `$HOME` 不能探 `$PATH`：`/bin/sh` 在 PATH 缺失时会自己合成一个默认值
- CLI 全局默认值 MUST NOT 在 `execute()` 里现算：e2e 的假进程是 `pm3 __sleep`，子进程环境被 `env_clear()` 清空后没有 `HOME`，「所有子命令都先解析配置路径」会让 sleeper 一启动就退出（症状是 e2e 里 app 显示 `stopped ↺1`）→ 交给 clap `default_value_t` 在构建期算，不读配置的命令自然不受影响
- fixture 里的 `create_dir_all` 会把「测试想要它缺失」的父目录造出来（`save_reports_a_missing_parent_directory` 一度失效）→ 造错误路径的 store/source fixture 必须接一个独立 root，别从被测路径 `parent()` 反推
- e2e 会泄漏 daemon 进程（tempdir 已删、进程仍在）：排查真机状态前先 `pgrep -f 'pm3 daemon --config /var/folders'` 清一遍，否则 `pgrep`/端口结果会误导
- 文件 IO 错误分支稳定触发：`create_dir_all` 失败 → 目标预置为文件；`rename` 失败 → 目标预置为 non-empty 目录；`remove_file` 失败 → path 是目录；`write` 失败 → 父目录 ENOENT

## 配置与路径约定

- `pm3.home`（`~/.pm3`）放运行时状态（socket/pid/logs/各服务工作目录）+ daemon 自己的 `config.yaml`（`service install` 落盘的那份，unit 的 `--config` 就指它；`cfg_dir` 由配置本身定义，放不进去）；`pm3.cfg_dir`（`~/.config/pm3`）只放每服务一份 `<name>.yaml`
- `dump.yaml` 只存 `services[].runtime`，启动参数全在 `cfg_dir/<name>.yaml`；`YamlDumpStore::load()` 经 `SpecSource` 把两者缝起来，服务文件缺失/损坏只 `warn` 跳过该条
- 服务配置里 MUST NOT 出现绝对路径：`$HOME/` 前缀折成 `${HOME}/`（加载时由 `substitute_env_vars` 展开）、`cwd` 不写（daemon 用 `<pm3.home>/<name>` 推导并建目录）、`script` 存裸名
- 写 `~/.pm3/config.yaml` 与 `cfg_dir/<name>.yaml` 共用 `svc::reconcile`：内容相同静默通过、不同则打 diff 并拒绝、`--force` 才覆盖；`service uninstall` 不删配置
- args 里指代「该服务自己的可写工作目录」MUST 用 `${PM3_SVC_CWD}`（命令行写裸 `PM3_SVC_CWD`，CLI 折叠成带花括号形式），MUST NOT 写 `${HOME}/.pm3/<name>`（那把 pm3 布局烧进了参数）；只在 args 生效，`cwd`/`writable_roots`/`script` 里写它不展开、会被相对路径校验直接拒；`pm3 describe` 显示的是展开后的真实路径，不能拿它当「配置无绝对路径」的证据
- `pm3.search_path` 是单一来源：既写进 launchd/systemd unit 的 PATH，也是 daemon 解析 app 程序名的搜索路径；CLI 早期校验必须用它而非 `std::env::var("PATH")`
- 子进程环境默认为空（`tokio_launcher` 有 `env_clear()`），所以 spawn 前必须已解析出绝对路径
- 给 `Pm3Config` 加字段要同步 6 处：根 `config.yaml`、`adapters/test_support/config_sections.rs`、`adapters/src/test_helpers/config_schema_test_helpers.rs`、`frameworks/test_support/config_fixtures.rs`、`frameworks/tests/common/mod.rs`、校验函数与 `every_error_variant` 表

## 分层

- `frameworks` 不得依赖 `usecases`/`entities`（arch_tests 强制）：内层类型一律从 `adapters` 的具名再导出取
