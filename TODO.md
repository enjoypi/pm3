# TODO

唯一任务清单，条目完成即删除。项目说明见 `docs/requirements.md`。

## 发布

- [ ] `install.sh` 真机验收：`HOME=$(mktemp -d)` 跑一次验首装路径，再跑一次验升级路径。**前提是仓库转 public**，私有状态下匿名下载全部 404

## Windows 收尾

- [ ] 修绿 `windows-e2e` 全量套件：`release.yml` 的该 job 跑全 workspace nextest，失败面是 fixture 写死 `/bin/sh`、`/tmp` 等 unix 路径。fail-fast 会掩盖完整失败面 → 先 `--no-fail-fast` 拿全清单再分批平台化 fixture（本机无 Windows，只能 CI 迭代）。修绿后把 `windows-e2e` 加回 `release.needs`（当前为发版临时摘除）
- [ ] Windows 真机验收：`service install --dry-run` 目检 → 注册/`/Query` → start/list/logs/stop → 注销重登自启 → `pm3 install` 换代 → uninstall 清场（`frameworks/tests/service_windows.rs` 已备好同路径 e2e，`release.yml` 在 tag 时自动跑）
  - 重点疑点：`pm3 install` 默认落位 `~\bin\pm3` 无 `.exe` 后缀（`adapters/src/install/layout.rs` 的 `DEFAULT_DESTINATION`），Windows 上无扩展名不可执行，验收时确认并修
- [ ] `schtasks /Query` 在非英文 locale 下输出解析失效（状态恒报 not running），需要时换 PowerShell `Get-ScheduledTask` 的对象化输出
