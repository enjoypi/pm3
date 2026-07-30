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
- 每个 `?` 的 Err 分支是独立 region，各需一条失败路径测试；「已证不可达」处用 `.expect()`（不产生 region）
- region 按**实例化**计数：lib+bin 双编译下，单测覆盖到的分支在 bin 副本仍算缺失 → 要么补 e2e 走真实 binary，要么让该分支只存在于一处
- 定位缺口：`cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'` 找文件；再 `--show-missing-lines`，若它无输出而 summary 有缺口，缺口就在 bin 副本
- 只有 trait 声明的文件进不了 lcov 会触发「生产文件缺失」→ 不写 blanket impl，让实现方显式 `impl Trait for X {}`
- 完整踩坑清单见 @AGENTS.md，改代码前先读

## 配置与路径约定

- `pm3.home`（`~/.pm3`）只放运行时状态（socket/pid/dump.yaml/logs/各服务工作目录）；`pm3.cfg_dir`（`~/.config/pm3`）放每服务一份配置
- 服务配置里 MUST NOT 出现绝对路径：`$HOME/` 前缀折成 `${HOME}/`（加载时由 `substitute_env_vars` 展开）、`cwd` 不写（daemon 用 `<pm3.home>/<name>` 推导并建目录）、`script` 存裸名
- `pm3.search_path` 是单一来源：既写进 launchd/systemd unit 的 PATH，也是 daemon 解析 app 程序名的搜索路径；CLI 早期校验必须用它而非 `std::env::var("PATH")`
- 子进程环境默认为空（`tokio_launcher` 有 `env_clear()`），所以 spawn 前必须已解析出绝对路径
- 给 `Pm3Config` 加字段要同步 6 处：根 `config.yaml`、`adapters/test_support/config_sections.rs`、`adapters/src/test_helpers/config_schema_test_helpers.rs`、`frameworks/test_support/config_fixtures.rs`、`frameworks/tests/common/mod.rs`、校验函数与 `every_error_variant` 表

## 分层

- `frameworks` 不得依赖 `usecases`/`entities`（arch_tests 强制）：内层类型一律从 `adapters` 的具名再导出取
