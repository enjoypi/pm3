# adapters — 双向格式转换与 Port 实现

Controller / Presenter / Gateway / DTO 全在这层。不放业务规则判断，不放用例编排。

## 文件地图

| 目录 | 内容 |
|---|---|
| `config/` | `Pm3Config` schema、loader、`substitute_env_vars` |
| `apps_file/` | 用户 apps 文件与单体服务文件的解析、`SpecSource`、`roots` |
| `http/` | daemon 侧 controller / routes / DTO（`ReplyDto`） |
| `persistence/` | `dump.yaml` 的 `yaml_store` 与 DTO |
| `presenter/` | `list` 表格、`describe`、reply 文案 |
| `process/` | `tokio_launcher` `kill_signaler` `ps_probe` `sha256_fingerprinter` `system_clock` `watcher` |
| `sandbox/` | `seatbelt`（macOS，含 `.sbpl`）/ `bwrap`（Linux）/ `wrapper` / `backend` |
| `schedule/` | `cron_scheduler` + `random_expand`（OpenBSD `~` 展开） |
| `service/` | `launchd` / `systemd` unit 渲染、`plan` `actions` `runner` `command` |
| 根文件 | `paths.rs` `program.rs` `startup.rs` `state.rs` `workspace.rs` |

## 本层规则

### 意图与运行态的缝合

- `cfg_dir/<name>.yaml` 存**意图**（零绝对路径，`${HOME}` 占位、`script` 存裸名、`cwd` 由 daemon 推导），`dump.yaml` 只存 `services[].runtime`；`SpecSource` 在 daemon 启动时把两者缝起来，服务文件缺失/损坏只跳过并 `warn`，MUST NOT 让整个 daemon 起不来
- `SpecSource::resolve_service` 用专属 `parse_service_file` 解析**单体格式**（顶层直接 `name:`/`script:`/…，不包 `apps:` 数组）并按文件名核对 `name`；`apps:` 数组只出现在用户手写的 apps 文件（`pm3 start apps.yaml`）

### 子进程不随父死

三处平台配置必须同时成立，缺一个都会在 daemon 换代时连带杀掉服务：launchd `AbandonProcessGroup`、systemd `KillMode=process`、`bwrap` **不加** `--die-with-parent`。

### 沙箱与路径

- macOS `sandbox-exec` 的 `subpath` 只认真实路径，`/var/...` 这类符号链接不匹配 → spawn 前必须 canonicalize `cwd` 与 `writable_roots`
- `materialise_workspace` 里展开 `${PM3_SVC_CWD}` MUST 排在 `spec.cwd = real_path(...)` **之后**；提前替换会把未 canonicalize 的 cwd 写进 args，正好复现上面那个陷阱
  回归测试：`src/tests/workspace_tests.rs::a_placeholder_expands_to_the_real_path_not_the_symlink` 与 `frameworks/tests/sandbox_isolation.rs::a_confined_app_can_write_through_the_cwd_placeholder`

### 进程

- `TokioProcessLauncher::wait` 会先把 `Child` 从 map 里 remove 再 await，所以「是否存活」必须另用一个 `live: HashSet<u32>` 跟踪（spawn 时插入、wait 返回后删除）
- `kill_signaler`（进程组信号）与 `ps_probe`（`ps -o lstart=` 身份令牌）的取值方式是硬约束，见根 `CLAUDE.md` 的「进程与信号」「身份指纹与接管」

### 调度

- `random_expand.rs` 在**每次 `arm_timer`** 把 `~`/`a~b`/`a~b/n` 展开成具体数字再交 croner，所以每次触发都重新摇；副产品是「早晚各一次」写成 `25~35 9,18 * * *` 时两次会落在不同分钟

### 服务管理器

- unit 文件位置由 OS 约定在**本层**派生（`~/Library/LaunchAgents/{label}.plist` / `~/.config/systemd/user/{label}.service`），**不进配置**——单个配置项无法同时对两个平台正确；`$HOME` 由 `frameworks` 注入，测试传 tempdir 就不会碰真实 `~`

### 序列化

- `serde_yaml2` 把空 `BTreeMap`/`Vec` 序列化成 `~`，再反序列化成 map 会失败 → 集合字段一律 `#[serde(default, skip_serializing_if = ...)]`
- `serde_yaml2::to_string` 输出人不可读（键带引号、缩进古怪）：要人可读可改的 yaml MUST 手写渲染器，用「encode → parse 回来相等」的 round-trip 测试兜底

### DTO

- 给 `ProcessView` 加字段会把 `DaemonReply::Described` 撑过 clippy `large_enum_variant` 阈值 → 在 enum 上加 `#[expect(..., reason = "one reply travels per CLI command")]`，别为过 lint 而 Box（会波及 controller/presenter 全链路）
