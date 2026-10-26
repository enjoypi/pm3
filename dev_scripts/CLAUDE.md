# dev_scripts — gates and development scripts

`just`'s complex recipes are all driven by the Bun/TypeScript here: `reap.ts` (leftover reaping), `monitor.ts`, `cargo_invocation.ts`（dev 的 cargo 唯一入口：固定 nightly，`--cov` 时注入插桩 env）, `bench.ts`, `cov.ts`（覆盖率门禁）.

