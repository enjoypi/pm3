# dev_scripts — gates and development scripts

`just`'s complex recipes are all driven by the Bun/TypeScript here: `cov.ts` + `coverage_gate.ts` (coverage gate), `reap.ts` (leftover reaping), `monitor.ts`, `rename.ts`, `cargo_invocation.ts`, `bench.ts`.

**Read this file before running `just cov` or troubleshooting the coverage gate.** Region-level fixes (how to make a specific line coverable) are in root `CLAUDE.md` "Coverage region discipline".

## TypeScript itself

- Run `bun install --frozen-lockfile` before `just typecheck` / `just test-scripts`
- `Bun.env.X` triggers TS4111 → write `Bun.env["X"]`; `Bun.spawn` does not accept `readonly string[]` → pass `[...command]`
- `just typecheck` bans `any` / non-null assertions / `ts-ignore`
- After changing the directory layout of `adapters/`, renaming types, or sinking constants into `config.yaml`, you MUST run `just test-scripts`: `tests/rust_contract.test.ts` reads Rust sources by **string paths** to guard "TS constants match Rust constants"; once a path goes stale, `sourceOf` returns an empty string for the missing file and only reports "no longer declared the way this guard reads it" without saying the file moved; after constants sink into `config.yaml` the guard must read the config's `:-` default values instead

## Gate operation (`just cov`, four metrics at 100%)

- The order MUST be `just lint` → `just cov`: `cov` runs only nextest, not clippy, so it cannot see problems like stale `#[expect]`; conversely `cov` can expose test-target `unused_imports` that `lint` misses (clippy's incremental cache may not recompile test targets) → run both
- `cargo-llvm-cov` ignores files whose path contains `tests/`; `test_helpers/` and `test_support/` **count** toward the gate — a `panic!` in a helper becomes an uncovered line
- After a change shifts line numbers you must `just cov --fresh`, otherwise leftover stale instantiations produce phantom `FNDA:0`
- The gate must be 100% on both macOS and Linux → any new platform difference is introduced by the current change, with two root-cause classes:
  - **Thin wrappers reading system paths** (`/proc/self` for `host_uid`): MUST extract an inner fn taking a `&Path` parameter (`owner_uid_of`); the production wrapper passes the constant, tests pass a tempdir and a nonexistent path — both arms reachable on either platform
  - **Tests that flip external state via sleep races** (fake `ps` reports alive first, the test sleeps, then marks gone): on a slow machine the first probe already lands after the flip, so branches like "waiters remain after polling" are only exercised on fast machines → MUST make the **fake program self-track its call count** (`if [ -f "$0.asked" ]; then exit 1; fi` + `touch`), answering alive the first time and gone the second, independent of machine speed

## Four self-rescue classes

| Symptom | Cause | Fix |
|---|---|---|
| Broad under-coverage; the same function appears in lcov as two `FN:` groups with different line numbers | `llvm-cov clean --workspace` does **not** delete stale test binaries in `deps/`; their coverage maps merge into the report at the old line numbers (criterion: in `ls -lT target/llvm-cov-target/release/deps` the binary timestamps predate this build) | `rm -rf target/llvm-cov-target` then `just cov --fresh` |
| All files 0%, thousands of `FNDA:0` | binary/profraw hash mismatch (triggered by crossing non-fresh runs with manual `cargo llvm-cov report`) | re-run `just cov --fresh` with no other cargo commands interleaved |
| Gate fails but not a single per-file detail is printed | the gap is regions and lcov carries no region data, so `findFilesBelowFullCoverage` naturally prints nothing | see "Locating region gaps" below |
| lcov details all green line by line yet the gate still reports `lines 382/383` | `DA:` is written **merged by source line** (union over instantiations), while the `LF/LH/BRF/BRH` the gate reads are summed from llvm-cov's per-function-instantiation-group statistics with `max` taken inside each group — two instantiations each covering half gives `max(1/2,1/2)=1/2` and reports missing. **Parsing lcov will never find it** (the `DA` count is naturally smaller than `LF`; this holds for every file in the repo and is not an anomaly signal) | see "Locating by instantiation" below |

### Locating region gaps

MUST run immediately after a `just cov --fresh` (with no other cargo commands in between):

```sh
cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'
```

With the file in hand, add `--show-missing-lines`. Three outcomes:

- No output and lines also missing → the gap is in the bin copy (lib+bin double compilation; regions counted per instantiation): add an e2e that drives the real binary, or make the branch exist in only one place
- No output and lines at 100% → the missing part is a pure `?`/short-circuit region; prime suspect is a newly added `?`
- When done, return to `--fresh`

### Locating by instantiation

```sh
cargo +nightly llvm-cov report --release --offline --json --output-path <f>
```

Take the entries in `data[0].functions[]` whose `filenames` include the target file:

- **Group by crate copy**: group by the `Cs<hash>_` in the name → within each group take `max(count)` per line over `regions[]` (`[lineStart,colStart,lineEnd,colEnd,count,fileId,…]`) → whichever group is 0 means **that copy** must also be exercised. `frameworks` has at least three copies counted simultaneously: the lib test, the `pm3` bin, and **each `frameworks/tests/*.rs` e2e binary links its own copy of the lib** (if the lib side is short, add unit tests; if the bin/e2e side is short, add cases under `frameworks/tests/`)
- **Finding branch gaps**: per instantiation, look in `branches[]` for entries with `b[4]==0 || b[5]==0` — when the same `line:col` appears twice, one carrying only true and the other only false, that's it

## Leftover cleanup

- **Automatic reap**: `just test` and `just cov` each run `reap.ts` once before and once after, sweeping leaked e2e daemons (TERM→KILL including descendants, deleting fixture tempdirs). It reaps only when all three guards hit: `ppid == 1` (a daemon of a running test has the test process as parent — untouched) + binary under `<repo>/target/` (real-machine `~/bin/pm3` untouched) + (config gone or containing the `pm3-e2e-never-installed`/`pm3-fixture` fingerprint) (manually mktemp'd homes untouched)
- Manual inspection is only needed when `just` itself is Ctrl-C killed. Clean up before inspecting real-machine state, otherwise `pgrep`/port results will mislead; child processes are no longer cleaned up with the test's process group once `process_group(0)` applies
- Listing leftovers MUST use `pgrep -x pm3` and then verify each with `ps -o pid=,args= -p <pid>`: `pgrep -f` also matches the shell that launched it. The signature of a leaked e2e daemon is `ppid=1` + `--config` pointing to a no-longer-existing tempdir (macOS `/var/folders/...`, Linux `/tmp/.tmp*`); the real-machine one points to `~/.pm3/config.yaml` — don't kill the wrong one
- **nextest-interrupt leftovers**: a flake cancels the remaining tests → `TempDir`'s Drop never runs, leaving e2e fixture directories under `$TMPDIR` (`config.yaml` + `home/{logs,service,pm3.sock}`). Locate them with `rg -l --hidden 'pm3-e2e-never-installed|pm3-fixture' "$TMPDIR" -g config.yaml` — `rg` skips hidden directories by default and these are exactly `.tmp*`, so omitting `--hidden` yields false negatives; match by the label fingerprint rather than the directory name so you don't delete the real-machine config
