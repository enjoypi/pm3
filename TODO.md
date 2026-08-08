# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

- [ ] 在 macOS 上跑一次 `just lint` + `just cov --fresh` 复核：四处平台性缺口的修法（`layout.rs` 的 `owner_uid_of`、`ps_probe.rs` 的重探测试、`watcher.rs` 的自计数 fake ps、`private_file.rs` 的 `fill`）都是在 Linux 上验证的，两平台应同为 100%

## 发布与用户安装方案

现状：仓库没有 README。装 pm3 两条路：clone 仓库跑 `just install`；或 GitHub Releases 下三平台二进制（macOS arm64、Linux x86_64/arm64，tag `v*` 触发 `.github/workflows/release.yml` 构建，产物带 LICENSE 与安装提示）。

- [ ] **`cargo install`**：要求**包名**就是用户敲的名字，而现在带 `[[bin]] pm3` 的包叫 `frameworks` ⇒ `cargo install pm3` 装不了。四个 crate 得改名（`pm3` + `pm3-entities` / `pm3-usecases` / `pm3-adapters`）并**全部发到 crates.io**——path 依赖不能发布，少一个都装不上。`entities`/`adapters` 这类通用名在 crates.io 上本就占不到，改名是必经之路
  - MUST NOT 用「合并成单 crate」绕开：`arch_tests` 的依赖方向强制是**靠 crate 边界**成立的，合并等于把它拆了
  - 还缺 crates.io 必需元数据：`license`（或 `license-file`）、`description`、`repository`、`readme`，四个 `Cargo.toml` 现在一个都没有 ⇒ 与下面的 README/LICENSE 两条绑在一起做
  - `dev_scripts/rename.ts` 是改**项目名**的模板脚本，不负责这次的 crate 改名
- [ ] **`cargo install --git <url> --bin pm3`**：不需要发布、不要求包名匹配，成本最低 ⇒ 可以先把这条写进 README 顶住，再慢慢推 crates.io
- [ ] **`curl … | sh` 一行装**：前提（CI 构建矩阵 + GitHub Releases 产物）已满足，缺安装器脚本本体。装完是裸二进制，紧接着跑 `pm3 install` 就能自己落位并注册开机自启
- [ ] 安装文档 MUST 写明运行时依赖：`/bin/ps` 与 `/bin/kill`（procps，缺了每次 daemon 重启全部服务被判探测失败而驱逐）、Linux 侧的 `bwrap`（缺了沙箱起不来）
- [ ] 可选分发面：Homebrew tap、`cargo-binstall`（后者跟着 Releases 产物白拿）

## README（中文）

- [ ] 仓库根还没有 README。README 写定位（极简 pm2 + 严格沙箱）、安装、快速上手（`start` / `list` / `logs` / `restart` / `service install`）、默认沙箱行为（只写自己 cwd、拒网、`read: minimal`）、`<name>.env` 的凭据约定、两个目录各放什么
  - 与 `docs/requirements.md` **分工要清楚**：那份是需求描述（为什么这样设计），README 是上手指南（怎么用）⇒ MUST NOT 复制粘贴，否则两份必然漂移
  - crates.io 发布要求 `readme` 与 `license` 字段 ⇒ 与上一节联动
  - README 内容：1. pm3是什么，为什么要新做个 pm3，设计理念是什么；2. 安装方法；3. 使用方法；4. 和 pm2 相比有什么优劣；5. 和 docker/podman 相比有什么优劣；6. 性能测试数据

## [ ] Windows Service
