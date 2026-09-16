# dev_scripts — gates and development scripts

`just`'s complex recipes are all driven by the Bun/TypeScript here: `reap.ts` (leftover reaping), `monitor.ts`, `rename.ts`, `cargo_invocation.ts`, `bench.ts`.

The coverage gate lives in the `rust-cov-100` skill (`scripts/precheck.ts` + `scripts/gate.ts`); never rebuild it here.
