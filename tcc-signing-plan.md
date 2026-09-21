# pm3 macOS TCC 反复弹窗根治：稳定签名身份 + install 幂等

## Context（诊断结论）

用户反映 pm3 经常弹「是否允许访问云盘目录或文档目录」，尽管已开完全磁盘访问（FDA）。三方调查 + 本机实测确诊：

1. **触发者**：`~/.config/pm3/gdrive.yaml` 以 `danger-full-access` 沙箱直接运行 Google Drive 客户端，它访问 `~/Library/CloudStorage` 时 TCC 按 responsible process 归账到 pm3 daemon，弹窗挂在「pm3」头上。FDA 本身覆盖云盘目录，所以弹窗 = FDA 没生效。
2. **FDA 为何没生效**：`~/.local/bin/pm3` 是 linker 自动加的 ad-hoc 签名（实测 `Signature=adhoc`、`TeamIdentifier=not set`）。TCC 对无稳定签名身份的 binary 按 cdhash 记账；每次源码/依赖/nightly 变化后 `just install` 原子覆盖 binary（`adapters/src/install/store.rs:25` `replace_binary` 无「相同则跳过」判断），cdhash 一变，旧 FDA 授权静默作废——设置面板勾选还在，对新二进制无效。迭代越频繁弹得越「经常」。
3. 次要因素：直接跑 `target/release/pm3` 触发 `ensure_daemon_running` 会 spawn 出 dev 路径、另一 cdhash 的 daemon，是另一个从未授权的 TCC 主体。

修复范围（用户已确认）：**1 自签名证书签名 + 3 install 幂等跳过**；gdrive 维持 pm3 托管（签名身份稳定后，一次 FDA 授权即永久覆盖云盘访问）。

## 方案

核心原理：TCC 对有效签名的 binary 按 designated requirement（证书身份）而非 cdhash 记账。自签名 codesign 证书无需付费 Developer ID 即可获得跨 rebuild 稳定的身份。关键约束：**本机与 CI 必须用同一个证书**（designated requirement 绑定证书叶子哈希，两个同名不同钥的证书 = 两个身份）。

### 1. 生成自签名证书（一次性，justfile 新 recipe）

`justfile` 新增 `signing-identity` recipe（内联 shell，与既有 install recipe 同风格）：

- `openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=pm3-local" -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=codeSigning"` 生成 key+crt 到临时目录
- `security import` 把 key 与 crt 导入 login keychain（首次 codesign 用钥时 macOS 会弹一次钥匙串访问确认，属预期）
- 末尾用 `security find-identity -v -p codesigning | rg pm3-local` 验证可见，并提示用户导出 p12 配 CI secrets（`PM3_CODESIGN_P12`、`PM3_CODESIGN_P12_PASSWORD`）

### 2. `just install` 构建后签名

`justfile:38-43` install recipe 在 `cargo build` 之后、`pm3 install` 之前插入：

- `security find-identity -v -p codesigning | rg -q pm3-local` 命中则 `codesign --force --sign "pm3-local" --identifier com.enjoypi.pm3 target/release/pm3`
- 未命中则 echo 警告继续（不阻塞无证书环境；identifier 取 launchd label 同款 `com.enjoypi.pm3`，见 `adapters/src/unit/launchd.rs`）

### 3. release CI 签名 macOS 产物

`.github/workflows/release.yml` build job 的 `package` 步骤前，仅当 `matrix.target == aarch64-apple-darwin` 且 secrets 存在时：

- base64 解码 `PM3_CODESIGN_P12` → 建临时 keychain → `security import`（带 `PM3_CODESIGN_P12_PASSWORD`）→ `security set-key-partition-list -S apple-tool:,apple: -s` → 同款 `codesign --force --sign "pm3-local" --identifier com.enjoypi.pm3`
- secrets 缺失则 echo 警告跳过（维持现状行为，不阻塞 release）
- 更新 release body 里「macOS：二进制未签名」那句说明

### 4. install 幂等跳过（TDD）

字节相同则不动 binary，消除无变化重装的 cdhash 失效面与旧 inode 窗口。

- 先写失败测试：
  - `adapters/tests/install_store_tests.rs`：新函数三用例（相同→true、不同→false、destination 不存在→false）
  - `frameworks/tests/install_tests.rs`：复用既有 fixture，验证字节相同时 install 不产出二进制备份、destination mtime 不变
- 实现：
  - `adapters/src/install/store.rs` 新增 `pub async fn binary_matches(source, destination) -> Result<bool, InstallError>`：NotFound→false，其余 IO 错误向上传播（不静默）
  - `frameworks/src/install.rs:79-81`：`binary_matches` 为 true 时跳过 `back_up(destination)` 与 `replace_binary`，emit 一行 `binary unchanged, kept <path>`；targets 的备份与 unit 重装流程不变
- 遵守覆盖率规则：复合布尔显式 `if`；改动后跑 `/rust-cov-100`

### 5. 记录坑（规则 31）

项目 `CLAUDE.md`「构建环境」一节追加：ad-hoc 签名 ⇒ TCC 按 cdhash 记账 ⇒ 任何 rebuild 后 install 即吊销 FDA；pm3-local 自签证书是本机与 CI 的共同身份，不可各自生成。

## 关键文件

| 文件 | 改动 |
|---|---|
| `justfile` | 新 `signing-identity` recipe；install recipe 加条件签名 |
| `.github/workflows/release.yml` | macOS 产物条件签名；改 release body 文案 |
| `adapters/src/install/store.rs` | 新增 `binary_matches` |
| `frameworks/src/install.rs` | 字节相同时跳过备份与替换 |
| `adapters/tests/install_store_tests.rs`、`frameworks/tests/install_tests.rs` | 先写失败测试 |
| `CLAUDE.md` | 记 TCC cdhash 坑 |

## 验证

1. `just test` + `just test-scripts` 全绿，`/rust-cov-100` 门禁过
2. `just signing-identity` 后 `security find-identity -v -p codesigning` 列出 pm3-local
3. 连跑两次 `just install`：第一次签名落位，第二次输出 `binary unchanged`；`codesign -dv ~/.local/bin/pm3` 显示 `Authority=pm3-local`
4. 一次性重授 FDA（系统设置 → 隐私与安全性 → 完全磁盘访问 → 重加 pm3），之后任意 rebuild + `just install` 不再弹云盘/文档目录窗口
5. 配置 CI secrets 后打一次 prerelease tag，确认 macOS tarball 内 binary 已签名

## 实施后

- 首次 commit 前把本文件从 `~/.claude/plans/` 移至项目目录（规则 35）
- commit message 单行 <64 字符：`fix: 自签名身份稳定 TCC 授权并幂等安装`
