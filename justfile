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

[doc("Windows 交叉编译检查：check + clippy（只编译不链接，msvc 链接器缺失不阻塞）")]
check-windows:
    cargo check {{ cargo_locked }} {{ cargo_common_flags }} --target x86_64-pc-windows-msvc
    cargo clippy {{ cargo_locked }} {{ cargo_common_flags }} --target x86_64-pc-windows-msvc --no-deps -- -D warnings

[doc("nextest；覆盖率门禁走 /rust-cov-100；跑前跑后自动 reap 泄漏的 e2e daemon")]
test *args:
    bun dev_scripts/reap.ts; cargo nextest run {{ cargo_locked }} {{ cargo_common_flags }} "$@"; status=$?; bun dev_scripts/reap.ts; exit $status

[doc("装到真机：opt-level 3 构建，有 pm3-local 身份则签名（TCC 授权跨重装稳定），再交给 pm3 install")]
install:
    CARGO_PROFILE_RELEASE_OPT_LEVEL=3 cargo build {{ cargo_locked }} -p frameworks --release
    src=target/release/pm3; \
    if command -v security >/dev/null 2>&1 && security find-identity -p codesigning | rg -q pm3-local; then \
    src=target/release/pm3.signed; \
    if [ ! -f "$src" ] || [ ! -f "$src.src" ] || ! cmp -s target/release/pm3 "$src.src"; then \
    cp target/release/pm3 "$src.src"; \
    cp "$src.src" "$src"; \
    codesign --force --sign "pm3-local" --identifier com.enjoypi.pm3 "$src"; \
    fi; \
    else \
    echo "just install: 无 pm3-local 签名身份（just signing-identity 生成），二进制保持 ad-hoc 签名，TCC 授权将随 cdhash 失效" >&2; \
    fi; \
    cfg="${PM3_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/pm3}/config.yaml"; \
    [ -f "$cfg" ] || cfg="${PM3_HOME:-$HOME/.pm3}/config.yaml"; \
    [ -f "$cfg" ] || { echo "just install: 找不到真机 config.yaml，MUST NOT 用仓内那份（它的 roots 全为空）" >&2; exit 1; }; \
    target/release/pm3 --config "$cfg" install "$src"

[doc("一次性：生成 pm3-local 自签名证书导入 login keychain；本机与 CI MUST 共用同一把")]
signing-identity:
    if security find-identity -p codesigning | rg -q pm3-local; then \
    security find-identity -p codesigning | rg pm3-local; \
    echo "signing-identity: pm3-local 已存在，无需重建"; \
    exit 0; \
    fi
    tmp=$(mktemp -d); \
    trap 'rm -rf "$tmp"' EXIT; \
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -subj "/CN=pm3-local" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,digitalSignature" \
    -addext "extendedKeyUsage=codeSigning" \
    -keyout "$tmp/pm3-local.key" -out "$tmp/pm3-local.crt"; \
    openssl pkcs12 -export -legacy -out "$tmp/pm3-local.p12" \
    -inkey "$tmp/pm3-local.key" -in "$tmp/pm3-local.crt" -passout pass:pm3-local; \
    security import "$tmp/pm3-local.p12" -k "$HOME/Library/Keychains/login.keychain-db" -P pm3-local -T /usr/bin/codesign; \
    security find-identity -p codesigning | rg pm3-local
    @echo "signing-identity: 已导入 login keychain；首次签名若弹钥匙串授权请选「始终允许」。CI 用同一把：钥匙串访问导出 p12，配 secrets PM3_CODESIGN_P12（base64）与 PM3_CODESIGN_P12_PASSWORD"

[doc("tail 服务日志并过滤：crash 匹配 panic 与致命信号，business 匹配 error 与 WARN/ERROR")]
monitor kind:
    @bun dev_scripts/monitor.ts "$@"

[doc("性能采集：临时 home 起 daemon，测冷启动/RSS/start 到 Online/list 热路径，输出 markdown 表格")]
bench:
    bun dev_scripts/bench.ts

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
