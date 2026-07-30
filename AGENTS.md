# AGENTS

pm3 的踩坑清单。改代码前先读，能省掉重复试错。任务清单在 `TODO.md`，规则在 `CLAUDE.md`。

## 覆盖率门禁（`just cov`，四指标 100%）

- `cargo-llvm-cov` 默认忽略路径中含 `tests/` 的文件，所以 `src/tests/*_tests.rs` 不计入；`src/test_helpers/*.rs` 与 `test_support/*.rs` **计入**，helper 里的 `panic!` 分支会变成未覆盖行
- 每个 thiserror variant 都要构造 + `.to_string()` 断言一次，否则该 `Display::fmt` match arm 的 region 不计入覆盖
- `.expect()` / `.unwrap_or(<常量>)` / `.unwrap_or_default()` 不产生本文件 region；「已证不可达」处用 `.expect()` 优于 `map_err` + `?`。反之每个 `?` 的 Err 分支都是独立 region，都需要一条失败路径测试
- `tracing::debug!(field = <表达式>)` 的表达式只在 subscriber 启用时求值，测试无 subscriber → 该行 region 不覆盖；MUST 先 `let x = <表达式>;` 再 `tracing::debug!(x)`
- `if cond { ... }` 块尾的 `}` 会生成一条独立 region，只有「进入块又走完」才算命中；若该路径不可达就改写成 `if !cond { return ... }` 的早返形式
- `tokio::select!` 展开出的不可达 region 无法覆盖；用一个 forward task 把两个 channel 汇成一个，主循环只 `recv()` 一个 queue
- 泛型/`impl Trait` 参数会按实例化各算一份 region：把 `shutdown: impl Future` 改成 `Pin<Box<dyn Future + Send>>` 可把实例化收敛为一份
- lib + bin 同 crate 时，lib 会被编译两份（lib-test 与 bin），bin 那份里没被调用的函数同样计未覆盖 → 需要 e2e 走到那条路径（例如 `pm3 stop` 后要多等 `kill_timeout_ms` 才会触发 `on_force_kill`）
- 只有 trait 声明 + blanket impl 的文件永远进不了 lcov，会触发「生产文件缺失」；去掉 blanket impl 改为各实现方显式 `impl Ports for X {}`
- `?` 的 Err region 可达性取决于调用顺序：`canonical_config_path` 排在 `load_and_parse_config` **之后**时其 Err 分支永不可达（文件已读成功 → canonicalize 必成功），把「路径解析」提到「读文件」之前才能覆盖
- 不可注入的系统读取（`std::env::current_exe()`）不要在函数体里直接 `?`；把 `io::Result<T>` 塞进注入的 context，用 `.as_ref().map_err(...)?` 消费，测试才能构造 Err 命中该 region
- 不可达的防御分支应**重写消除**，而非加测试掩盖
- 改过 struct 字段/行号后覆盖率数据会残留旧实例化，出现 `FNDA:0` 的幽灵条目 → 必须 `just cov --fresh`

## Rust / clippy

- 同一 `test_support/*.rs` 文件 MUST NOT 被两处 `#[path]` 重复挂载（clippy `duplicate_mod`）；统一在 `lib.rs` 以 `#[cfg(test)] pub(crate) mod` 挂载一次
- test_helper 的请求构造器 MUST NOT 与 handler 同名（`get`/`post`/`delete` 在 `use super::{test_helpers::*, *}` 下二义），用 `get_from`/`post_to`/`delete_at`
- 只有 `Ok` 分支的测试 fixture 会触发 clippy `unnecessary_wraps`：fixture 返回裸值，调用处再 `Ok(...)`
- clippy 会报 `similar_names`（`launcher` 与 `launched`、`receiver` 与 `received`）、`significant_drop_tightening`、`shadow_unrelated`
- 跨 async 边界的回调参数要写 `&(dyn Fn(&str) + Send + Sync)`，否则外层 future 不是 `Send`
- `.collect::<Vec<_>>().join("")` 触发 clippy `unnecessary_join` → 改 `.collect::<String>()`
- `frameworks` MUST NOT 依赖 `usecases`/`entities`（arch_tests 强制）：所有内层类型都从 `adapters` 的具名再导出取

## 运行时行为

- `serde_yaml2` 把空 `BTreeMap`/`Vec` 序列化成 `~`，再反序列化成 map 会失败 → 集合字段一律 `#[serde(default, skip_serializing_if = ...)]`
- macOS 上 `sandbox-exec` 的 `subpath` 只认真实路径，`/var/...` 这类符号链接不匹配 → spawn 前必须 canonicalize `cwd` 与 `writable_roots`
- `TokioProcessLauncher::wait` 会先把 `Child` 从 map 里 remove 再 await，所以「是否存活」必须另用一个 `live: HashSet<u32>` 跟踪（spawn 时插入、wait 返回后删除）
- `sh -c "trap '' TERM; sleep 30"` 在被 pm3 spawn 时并不能可靠忽略 SIGTERM（手工 shell 与 python spawn 都能，pm3 路径不能，原因未查明）→ 不要用它当「顽固进程」测试靶子；要覆盖强杀路径就直接调 `on_force_kill`，或先用假的 `on_exit` 让表以为进程已退出
- 集成测试断言「依赖先启动」不能看应用自己写的文件（并发写有竞态），要把 `log_level` 调成 debug 后从 `pm3.log` 里读 `"action":"spawn"` 的顺序
- 测「调用外部服务管理器」（`launchctl`/`systemctl`/`loginctl`）用临时目录里的 `#!/bin/sh` 脚本 + `set_permissions(0o755)` 当替身，可同时控制 stdout 与退出码；真实二进制只用 `/usr/bin/true`、`/usr/bin/false`、`/nonexistent/...`，**绝不**在测试里调真的 `launchctl`/`systemctl`
- 服务 unit 文件位置由 OS 约定在 adapters 里派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），不进配置——单个配置项无法同时对两个平台正确；`$HOME` 由 frameworks 注入，测试传 tempdir 就不会碰真实 `~`
- 文件 IO 错误分支稳定触发：`create_dir_all` 失败 → 目标预置为文件；`rename` 失败 → 目标预置为 non-empty 目录；`remove_file` 失败 → path 是目录；`write` 失败 → 父目录 ENOENT
