# dev_scripts — 门禁与开发脚本

`just` 的复杂 recipe 都由这里的 Bun/TypeScript 驱动：`cov.ts` + `coverage_gate.ts`（覆盖率门禁）、`reap.ts`（收残留）、`monitor.ts`、`rename.ts`、`cargo_invocation.ts`、`bench.ts`。

**跑 `just cov` / 排查覆盖率门禁前先读本文件。** region 级的修法（怎么让某一行可被覆盖）在根 `CLAUDE.md` 的「覆盖率 region 纪律」。

## TS 本身

- `just typecheck` / `just test-scripts` 前先 `bun install --frozen-lockfile`
- `Bun.env.X` 触发 TS4111 → 写 `Bun.env["X"]`；`Bun.spawn` 不收 `readonly string[]` → 传 `[...command]`
- `just typecheck` 禁 `any` / 非空断言 / `ts-ignore`
- 改 `adapters/` 的目录结构、重命名类型、或把常量下沉进 `config.yaml` 后 MUST 跑 `just test-scripts`：`tests/rust_contract.test.ts` 靠**字符串路径**读 Rust 源码来守「TS 常量与 Rust 常量一致」，路径失效后 `sourceOf` 对缺失文件返回空串、只报「no longer declared the way this guard reads it」而不说文件已搬走；常量下沉进 `config.yaml` 后守卫要改读配置的 `:-` 默认值

## 门禁运行（`just cov`，四指标 100%）

- 顺序 MUST 是 `just lint` → `just cov`：`cov` 只跑 nextest 不跑 clippy，`#[expect]` 失效这类问题它看不见；反过来 `cov` 又能暴露 `lint` 漏报的 test target `unused_imports`（clippy 增量缓存可能不重编测试目标）→ 两个都要跑
- `cargo-llvm-cov` 忽略路径含 `tests/` 的文件；`test_helpers/` 与 `test_support/` **计入**门禁，helper 里的 `panic!` 会变成未覆盖行
- 改动令行号位移后必须 `just cov --fresh`，否则残留旧实例化产生幽灵 `FNDA:0`
- 门禁在 macOS 与 Linux 都应 100% → 新出现的平台差异一律是本次改动引入的，两类根因：
  - **读系统路径的薄包装**（`/proc/self` 之于 `host_uid`）：MUST 抽出接 `&Path` 参数的 inner fn（`owner_uid_of`），生产包装传常量、测试传 tempdir 与不存在的路径，两条臂在哪个平台都可达
  - **靠 sleep 竞争切换外部状态的测试**（fake `ps` 先报 alive、测试 sleep 后再 mark_gone）：慢机器上第一轮探测就已落在切换之后，于是「轮询后仍有等待者」这类分支只在快机器上被走到 → MUST 让 **fake 程序自持调用计数**（`if [ -f "$0.asked" ]; then exit 1; fi` + `touch`），第一次答 alive、第二次答 gone，与机器速度无关

## 四类自救

| 症状 | 原因 | 修法 |
|---|---|---|
| 大面积欠覆盖，lcov 里同一函数出现两组行号不同的 `FN:` | `llvm-cov clean --workspace` **不删** `deps/` 里的陈旧测试二进制，它们的 coverage map 以旧行号合并进报告（判据：`ls -lT target/llvm-cov-target/release/deps` 里二进制时间戳早于本次构建） | `rm -rf target/llvm-cov-target` 再 `just cov --fresh` |
| 所有文件 0%、`FNDA:0` 上千条 | 二进制与 profraw 哈希错位（非 fresh 与手动 `cargo llvm-cov report` 交叉跑会触发） | 重跑 `just cov --fresh`，中途不插任何其他 cargo 命令 |
| 门禁失败却一行文件明细都没打 | 缺口是 region 而 lcov 不含 region 数据，`findFilesBelowFullCoverage` 自然无输出 | 见下「定位 region 缺口」 |
| lcov 明细逐条全绿而门禁仍报 `lines 382/383` | `DA:` 是**按源码行合并**后写的（多实例化取并集），门禁读的 `LF/LH/BRF/BRH` 是 llvm-cov 按函数实例化组统计后相加、组内取 `max`——两份实例化各覆盖一半，`max(1/2,1/2)=1/2` 就报缺失。**解析 lcov 永远查不出来**（`DA` 条数天然少于 `LF`，全仓库每个文件都如此，不是异常信号） | 见下「按实例化定位」 |

### 定位 region 缺口

MUST 紧跟在一次 `just cov --fresh` 之后（中途不插其他 cargo 命令）跑：

```sh
cargo +nightly llvm-cov report --release --summary-only | awk 'NR>2 && $3+0>0'
```

拿到文件后再 `--show-missing-lines`。三种结果：

- 无输出且 lines 也缺 → 缺口在 bin 副本（lib+bin 双编译，region 按实例化计数）：补 e2e 走真实 binary，或让分支只存在于一处
- 无输出而 lines 100% → 缺的是 `?`/短路的纯 region，重点怀疑新加的 `?`
- 查完回到 `--fresh`

### 按实例化定位

```sh
cargo +nightly llvm-cov report --release --offline --json --output-path <f>
```

取 `data[0].functions[]` 里 `filenames` 含目标文件的项：

- **按 crate 副本分组**：按名字里的 `Cs<hash>_` 分组 → 每组内对 `regions[]`（`[行起,列起,行止,列止,count,fileId,…]`）逐行取 `max(count)` → 哪一组为 0 就得让**那份副本**也走到。`frameworks` 至少有三份副本同时被计量：lib test、`pm3` bin、以及**每个 `frameworks/tests/*.rs` e2e 二进制各链一份 lib**（lib 侧缺补单测，bin/e2e 侧缺补 `frameworks/tests/` 用例）
- **找分支缺口**：逐实例化找 `branches[]` 里 `b[4]==0 || b[5]==0` 的项——同一 `line:col` 出现两条、一条只有 true 一条只有 false，就是它

## 残留清理

- **自动 reap**：`just test` 与 `just cov` 跑前跑后各执行一次 `reap.ts`，收走泄漏的 e2e daemon（连带子孙进程 TERM→KILL、删 fixture 临时目录）。三守卫全中才收：`ppid == 1`（在跑测试的 daemon 父进程是测试进程，不动）+ 二进制在 `<repo>/target/` 下（真机 `~/bin/pm3` 不动）+（config 已消失或含 `pm3-e2e-never-installed`/`pm3-fixture` 指纹）（手工 mktemp 的 home 不动）
- 只有 `just` 本身被 Ctrl-C 杀掉时才需要手工排查。排查真机状态前先清一遍，否则 `pgrep`/端口结果会误导；子进程自 `process_group(0)` 起不再随测试进程组被连带清理
- 列残留 MUST 用 `pgrep -x pm3` 再逐个 `ps -o pid=,args= -p <pid>` 核对：`pgrep -f` 会把发起它的 shell 一起匹配进来。泄漏的 e2e daemon 特征是 `ppid=1` + `--config` 指向已不存在的 tempdir（macOS `/var/folders/...`、Linux `/tmp/.tmp*`），真机那份指向 `~/.pm3/config.yaml`，别杀错
- **nextest 中断残留**：flake 触发取消剩余测试 → `TempDir` 的 Drop 跑不到，`$TMPDIR` 留下 e2e fixture 目录（`config.yaml` + `home/{logs,service,pm3.sock}`）。定位用 `rg -l --hidden 'pm3-e2e-never-installed|pm3-fixture' "$TMPDIR" -g config.yaml`——`rg` 默认跳过隐藏目录而这些正是 `.tmp*`，漏 `--hidden` 会得到假阴性；按 label 指纹而非目录名匹配，才不会误删真机配置
