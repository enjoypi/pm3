set positional-arguments

cargo_locked := "--locked"
cargo_common_flags := "--all-targets --workspace --release"
half_of_cpus := shell('n=$(( $(getconf _NPROCESSORS_ONLN) / 2 )); [ "$n" -ge 1 ] || n=1; echo "$n"')
forbidden_ts_syntax := ':\s*any\b|\bas\s+any\b|<\s*any\s*[,>]|,\s*any\s*>|\bany\s*\[\]|[\w\)\]]!\s*([.\[;,)}]|$)|@ts-(ignore|expect-error|nocheck)'

export LC_ALL := "C.UTF-8"
export CARGO_BUILD_JOBS := env_var_or_default("CARGO_BUILD_JOBS", half_of_cpus)
export CARGO_FLAGS := cargo_locked + " " + cargo_common_flags

[doc("列出全部可用命令")]
help:
    just --list

[doc("编译 workspace")]
build *args:
    cargo build {{ cargo_locked }} {{ cargo_common_flags }} "$@"

[doc("格式化全部 Rust 源码，nightly rustfmt 才能重排 import")]
fmt:
    cargo +nightly fmt --all

[doc("clippy 四组 lint 全开，任何 warning 即失败")]
lint *args:
    cargo clippy {{ cargo_locked }} {{ cargo_common_flags }} --no-deps "$@" -- -D warnings

[doc("裸 nextest，不含覆盖率门禁；日常验收用 just cov")]
test *args:
    cargo nextest run {{ cargo_locked }} {{ cargo_common_flags }} "$@"

[doc("覆盖率门禁：四指标 + lcov 真值 plate + 生产文件完整性自检；--fresh 清 workspace 重算")]
cov *args:
    bun dev_scripts/cov.ts "$@"

[doc("装到真机：opt-level 3 构建后交给 pm3 install（备份、原子换二进制、重装 unit、核对接管）")]
install:
    CARGO_PROFILE_RELEASE_OPT_LEVEL=3 cargo build {{ cargo_locked }} -p frameworks --release
    target/release/pm3 --config config.yaml install target/release/pm3

[doc("tail 服务日志并过滤：crash 匹配 panic 与致命信号，business 匹配 error 与 WARN/ERROR")]
monitor kind:
    @bun dev_scripts/monitor.ts "$@"

[doc("模板改名：全仓当前项目名替换为新 crate 名，随后跑 cargo check 验证")]
rename new_name:
    bun dev_scripts/rename.ts "$@"

[doc("dev_scripts 的 TypeScript 单元测试")]
test-scripts *args:
    bun test dev_scripts/tests "$@"

[doc("TypeScript 严格类型检查，并禁止 any 与非空断言与 ts-ignore")]
typecheck:
    bun x tsc --noEmit
    @rg -n {{ quote(forbidden_ts_syntax) }} dev_scripts; status=$?; if [ "$status" -ne 1 ]; then exit 1; fi
