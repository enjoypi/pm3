# dev_scripts — gates and development scripts

`just`'s complex recipes are all driven by the Bun/TypeScript here: `reap.ts` (leftover reaping), `monitor.ts`, `cargo_invocation.ts`（dev 的 cargo 入口，`--cov` 时注入插桩 env；工具链由 `rust-toolchain.toml` 定）, `bench.ts`, `cov.ts`（覆盖率门禁）.

