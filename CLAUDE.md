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
- 只有 trait 声明的文件进不了 lcov 会触发「生产文件缺失」→ 不写 blanket impl，让实现方显式 `impl Trait for X {}`
- 完整踩坑清单见 @AGENTS.md，改代码前先读

## 分层

- `frameworks` 不得依赖 `usecases`/`entities`（arch_tests 强制）：内层类型一律从 `adapters` 的具名再导出取
