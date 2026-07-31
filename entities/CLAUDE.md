# entities — 企业级业务规则

无 I/O、无 async、无框架依赖（`serde`/`thiserror` 的 derive 除外）。

## 文件地图

| 文件 | 内容 |
|---|---|
| `process/spec.rs` | `AppSpec` 与 `validate_spec` |
| `process/status.rs` | `ProcessStatus` 状态机（`is_running` / `is_settled`） |
| `process/restart.rs` | 重启策略与熔断计数 |
| `process/runtime.rs` | 运行态字段（pid、身份令牌、重启次数） |
| `process/depgraph.rs` | 依赖启动序与环检测 |
| `sandbox/policy.rs` | `SandboxPolicy`：`writable_roots` / `derived_roots` / `granted_roots()` |

## 本层规则

- 熔断判定是 `unstable_restarts >= max_restarts`（对齐 pm2 `God.js`），MUST NOT 改回 `>`
- `ProcessStatus` 新增变体时 `is_running()` 与 `is_settled()` 都要重新审：两者不是互补关系，`Stopping` 同时不满足二者
- `SandboxPolicy` 的 `writable_roots` / `derived_roots` 之分是「daemon 换代不误判 respawn」的地基，改动前先读根 `CLAUDE.md` 的「身份指纹与接管」；加字段的波及范围见根「改动波及清单」
