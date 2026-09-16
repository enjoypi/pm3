# pm3: XDG 布局 + 全局环境变量 + list/describe 改进

## Context

真机 `pm3 list` 输出 101 字符宽、19 行,暴露三类问题,诊断后合并为一份改动。

### 诊断结论(已用日志逐条坐实)

**caddy ↺1415 的真相**:`autorestart: false` 是 09-15 20:08:52 才手工追加进 `caddy.yaml` 的(位于文件第 18 行,排在 `sandbox:` 块后)。

| 区间 | `action="start"` 条数 | `restart_time` |
|---|---|---|
| 04:44:49Z ~ 12:08:52Z | 1419 | 0 → **1415** |
| 12:08:52Z 之后 | 152 | 冻结在 1415 |

前段只有 `max_restarts: 0`(= 无限制),`autorestart` 继承默认 `true` → 亚秒级崩溃循环 7.5 小时(日志节奏 `11:59:47.867/48.872/49.259/49.825`,单小时 1396 次),由 `supervise.rs:113 count_restart` 逐次累加。后段 152 次是 tunnel-health 的 `pm3 restart`,走 `restart.rs:29` settled 分支**完全不计数**。

**liveness 一次都没触发过**:`query.rs:181 liveness_watch_list` 过滤 `status == Online`,caddy 是 errored,从不进名单。

**崩溃根因**(业务侧):`env_clear()` 后无 `XDG_DATA_HOME`,caddy 回落到 `~/Library/Application Support/Caddy`(未授权)→ 读 root cert EPERM。日志 1661 条同一错误。ssh-tunnel-r ↺165 是下游受害者(转发目标 `127.0.0.1:8443` 是 caddy 监听口)。cloudflared ↺859 是历史账 + 网络抖动。

**mihomo-global `unstable=17 > max_restarts=15` 却 online**:GiveUp 时 `unstable_restarts` 照涨而 `restart_time` 冻结(`supervise.rs:116-121`),且 `mark_launched` 复活时**不清零**,清零只发生在下一次退出的 `decide_restart:45`。所以一个健康运行 20 小时的服务会一直挂着陈旧值。

**pm3 未中 pm2 缺陷⑥**(cron 重启抹掉计数):`reset_restarts` 全仓只有 `reset.rs:13` 一个调用点。但中了镜像缺陷——cron/手动 restart 既不清零也**不累加**。

### 三个待解问题

1. **备份不完整**:`~/.pm3/` 混放设置(`config.yaml`)与状态(`dump.yaml`/`logs/`/socket/pid)。只备份 `~/.config` 无法恢复。
2. **无全局 env**:每个服务重复声明 `XDG_*`/`TZ`。caddy 的失败直接证明了必要性。
3. **list 太长且看不出病因**:101 字符宽,`sandbox` 一列独占 22(19 行中 18 行带 `+net`,近乎恒真);`↺` 显示历史累计而决定断路器的 `unstable_restarts` 不可见。

### 顺带发现的漏洞

XDG 后 `config.yaml` 与服务 yaml 同目录,而 `service_file_of(cfg_dir,"config")` = `cfg_dir/config.yaml` → **`pm3 start --name config --force <prog>` 覆写 daemon 配置**。与 `.enc` 后缀被保留同构。

## 已定决策

| 议题 | 决策 |
|---|---|
| 范围 | XDG 全面重构 + 全局 env + describe 脱敏 + list 改进,一次做完 |
| 全局 env 落点 | 设置侧(config 根),随 `~/.config` 备份;支持 sops |
| 全局 env 进指纹 | **否**(部署属性非进程身份)。代价:改后需手动 restart |
| env 优先级 | `HOME`(注入) < 全局 < 应用 |
| describe 脱敏 | 值 ≥16 字节显 `头4..尾4 <字节数>`,<16 只显 `.. <字节数>`;默认展开 + 标注来源 |
| `↺` 列 | 只显 `unstable_restarts`,`restart_time` 进 describe |
| 失败通知 | 本轮不做 |
| liveness 与 autorestart | liveness **跳过** `autorestart: false` 的服务(现状是"只杀不救") |
| 脱敏字符数 | 固定 4 字符 + 阈值 16 字节,不可配(安全策略不该能被配置削弱;也避免把 config 依赖拖进 entities) |
| 默认沙箱模式 | **不改**(`workspace-write`+`minimal`+`no network` 已是"能干活的最小集";真机 18/19 的 `+net` 是运维显式开的) |
| 上线节奏 | 6 个 commit 全做完再上真机 |

---

## A. XDG 布局重构

### A-1 四类根

`Pm3Paths` **加字段不拆 struct**(生产侧只 11 处读字段,测试侧 100+ 处;保持 8 个字段名不变则测试一行不动)。

```rust
pub struct Pm3Roots { config, state, runtime, data: PathBuf }
impl Pm3Roots {
    pub fn single(root: &Path) -> Self      // legacy:四根全 = root
    pub const fn split(...) -> Self
}
pub fn resolve_paths(roots: Pm3Roots) -> Pm3Paths   // 唯一组装点,一个函数体
```

`Pm3Paths` 新增 `roots: Pm3Roots` + `apps_dir`(state/apps)+ `backups_dir`(data/install-backups)。

| 现状(都在 `~/.pm3/`) | 归属 | 备份 |
|---|---|---|
| `config.yaml` + 服务 yaml + 全局 env | config 根 | ✅ |
| `dump.yaml` / `logs/` / `pm3.log` | state 根 | ❌ |
| 服务 cwd(`apps/<name>`) | state 根 | ⚠️ |
| socket / pid / lock / pipe.secret | runtime 根 | ❌ |
| `install-backups/` | data 根 | ❌ |

### A-2 解析优先级(`adapters/src/paths.rs`,全部纯函数只收 `Option<&str>`)

config/state/data 三档同构:`$PM3_<X>_DIR` > `$XDG_<X>_HOME/pm3` > `~/.config|.local/state|.local/share` + `/pm3`。第 1 档是整根绝对路径不追加 `pm3`。

runtime 四档(遵循 XDG 规范对 `$XDG_RUNTIME_DIR` 的要求):`$PM3_RUNTIME_DIR` > `$XDG_RUNTIME_DIR/pm3` > unix 且 `/run/user/<uid>` **实际存在** → `/run/user/<uid>/pm3` > `<state>/run` **并打 warn**(规范明说 "fall back to a replacement directory with similar capabilities and print a warning message")。

四条硬约束:
- **绝不用 macOS 的 `$TMPDIR`**。它看起来最像(per-user、会被清理),但根 CLAUDE.md:102:launchd 起的 daemon 有 `TMPDIR`、shell 起的没有 → CLI 与 daemon 算出**不同 socket 路径** → 拉起第二个不受管 daemon → 两个 supervisor 写一份 `dump.yaml`。第 4 档从 HOME 派生(unit 一定导出 HOME),三种上下文一致。macOS 确实没有符合规范的目录(无 `XDG_RUNTIME_DIR`、无 `/run/user`、`/var/run` 需 root),第 4 档是唯一确定性选择。它违反规范的 "MUST not survive reboot",但**不是回退**——pm3 今天的 socket 就在 `~/.pm3/`,靠 stale-socket 自愈处理重启残留。改用 `/tmp/pm3-<uid>` 虽满足"不跨重启",但 `/tmp` 全局可写 ⇒ 别的用户可抢先创建该目录 ⇒ 需新增一整套所有权校验,在解决不存在的问题时引入新攻击面。
- **socket 与 pid 文件必须设 sticky 位**。规范:"Files in this directory MAY be subjected to periodic clean-up... the 'sticky' bit should be set on the file"。**这条对 pm3 是灾难级的,且只在采用 `XDG_RUNTIME_DIR` 后才出现**:空闲 6 小时以上的 daemon,其 socket 的 atime 不被任何东西更新(周期 tick 是内存采样与 liveness,都不碰 socket)⇒ systemd-tmpfiles 清掉它 ⇒ socket 文件消失但 daemon 仍活(持有已 unlink 的 inode)⇒ 下一条 CLI 命令 connect 得 NotFound ⇒ `ensure_daemon_running` 判定无 daemon 并 spawn 新的 ⇒ **两个 supervisor 写同一份 `dump.yaml`**。设 sticky 位比"每 6 小时 touch"好:不需要新 tick、新 JoinHandle、新覆盖率 region。正面效果:`pm3.pid` 不跨重启存活,正是 `PidTrust` 想要的语义。
- 第 3 档**必须探测目录存在**。现有 `runtime_dir_of`(`unit/spec.rs:164`)只看 uid,macOS 上返回不存在的路径——今天只喂 `systemctl --user` 所以无害,拿去 bind socket 就是 macOS 全线炸。新函数收 `probe: fn(&Path)->bool`,生产传 `|p| p.is_dir()`,测试传固定值,**两分支在两平台都可达**。`runtime_dir_of` 本身不动(语义不同,不要合并)。
- **socket 长度校验**:macOS `SUN_LEN` ≤ 104 字节。新增 `PathError::SocketTooLong`,在 `bind_uds` 之前检查,否则用户设了深路径只会得到 tokio 的裸报错。实测第 4 档 `/Users/zhoufan/.local/state/pm3/run/pm3.sock` = 44 字节、Linux 第 3 档 `/run/user/1000/pm3/pm3.sock` = 26 字节,都安全。
- runtime 根本身必须 0700 且属当前用户(规范要求),复用 `restrict_to_owner`。

macOS 与 Linux 刻意一致(一套布局 = 一套测试 + 一份迁移文档),分歧点由存在性探测吸收。**此决定写进根 CLAUDE.md**,否则半年后有人改成 macOS 原生。

### A-3 `PM3_HOME` 存在 ⇒ legacy 单根模式

```
PM3_HOME 已设 → Pm3Roots::single(expand_home(PM3_HOME)),四个 XDG 变量全忽略
PM3_HOME 未设 + pm3.home 非空 → single(expand_home(pm3.home))
两者都空 → XDG 四根
```

三个收益:20+ 个 e2e 零改动(每个 e2e 用自己的 `PM3_HOME`);`PM3_HOME=/srv/pm3` 原样工作;**真机升级是布局 no-op**(真机 `config.yaml` 不被升级重写,仍写 `home: "${PM3_HOME:-~/.pm3}"` → 永走 legacy → 19 个服务照常 adopt,零中断)。

仓内 `config.yaml` 默认值改空(`home: "${PM3_HOME:-}"` 等),否则 XDG 永不激活。**yaml 里写不出 XDG 默认值**:`substitute_env_vars` 不递归展开默认值(根 CLAUDE.md:206),`${PM3_STATE_DIR:-${XDG_STATE_HOME}/pm3}` 会把内层原样留下 → XDG 解析必须在 Rust 里做。此条写进 `adapters/CLAUDE.md`。

`cfg_dir` 空 ⇒ = config 根(XDG 下即 `~/.config/pm3`,与真机现值一致,服务文件一个不用动)。

根 CLAUDE.md:201 那条"config.yaml 只能待在 pm3.home"的理由在 XDG 下消失(config.yaml 待在 config 根、由 env 发现;`cfg_dir` 默认也是 config 根)→ 规则重写,并补上保留名理由。

`default_config_path` 签名扩展为收四个 env 参数。调用点只有 `cli.rs:52`(clap `default_value_t`,**必须继续在 build 时算好**,否则 `pm3 __sleep` 在 `env_clear()` 后无 HOME 会当场退出)。

### A-4 沙箱隐藏根从 2 个变 4 个

`file.rs:404 pm3_owned_roots` 改藏四类根,`SpecDefaults` 加 `state_dir`/`runtime_dir`/`data_dir`/`apps_dir`。

runtime 第 4 档 `<state>/run` **嵌在 state 里**,而 `dedup_roots` 只按完全相等去重 → 新增 `dominant_roots`(复用 `entities::covers_path` 丢掉被覆盖的根),放 `apps_file/roots.rs`。

**PM3-44 铁律逐条校验**(服务 cwd = `<state>/apps/<name>` 嵌在隐藏根 state 里,与今天 legacy 下"cwd 在 pm3.home 里而 pm3.home 是隐藏根"**完全同构**,不是新形状):
- `covers_path(granted=cwd, hidden=state)` = false ⇒ `carveout_for` **不生成** carveout。正确——PM3-44 的事故正是无条件挂 carveout 让"granted 嵌在 hidden 里"的授权被 `require-not` 匹配祖先而整条作废
- SBPL last-match-wins:cwd 的 writable allow 排在 state 的 deny **之后** ⇒ 可写
- `<state>/dump.yaml`、`<state>/run/pm3.sock` 命中 deny 且不被任何 granted 覆盖 ⇒ 保持隐藏
- bwrap:`nested_in(hidden=[state], granted=[cwd, logs])` → `encloses(granted, state)` false ⇒ 不二次 tmpfs ⇒ 保留 bind
- `path-ancestors` 已成对写(`rules()`),cwd 祖先链多两级仍在覆盖内 ⇒ 逐级 stat 的服务不会 EPERM

**必须新增 e2e**(`frameworks/tests/sandbox_isolation.rs`):XDG 模式下 workspace-write 服务能写 cwd、不能读 `<state>/dump.yaml`、不能读 config 根、`nc -z -U <socket>` 退出 1。单测只验 profile 文本,验不了内核。

### A-5 unit 必须固化四根为 `PM3_*`

**XDG 下最致命的 bug,也是"`XDG_*` 能否从 shell env 拿"的答案**:能拿到但**不可靠**。真机 plist 实际只导出三个变量:

```
HOME, PATH, PM3_SOPS_IDENTITY_FILE
```

因为 `host_pm3_env()` 只**过滤** `PM3_` 前缀,而 `XDG_DATA_HOME` 不带这个前缀 ⇒ **launchd 启动的 daemon 完全看不到它**,而你终端里的 CLI 能看到 ⇒ CLI 算出 `$XDG_STATE_HOME/pm3`、daemon 算出 `~/.local/state/pm3` ⇒ **socket 路径分叉** ⇒ CLI 连不上 ⇒ `ensure_daemon_running` 拉起第二个不受管 daemon ⇒ 两个 supervisor 写一份 `dump.yaml`。这与 CLAUDE.md 已记录的 `${PM3_HOME:-~/.pm3}` 那次是同一个坑。

**解法:`XDG_*` 只在 `pm3 startup` 时读一次,解析成绝对路径固化进 unit。**

`host_pm3_env(roots: &Pm3Roots)`:先过滤 `PM3_`,再 upsert 四条 `PM3_{CONFIG,STATE,RUNTIME,DATA}_DIR`,最后过 `pm3_variables()` **排序**(不排序 `reconcile` 每次判 Stale)。plist 变成:

```xml
<key>PM3_CONFIG_DIR</key>  <string>/Users/zhoufan/.config/pm3</string>
<key>PM3_STATE_DIR</key>   <string>/Users/zhoufan/.local/state/pm3</string>
<key>PM3_RUNTIME_DIR</key> <string>/Users/zhoufan/.local/state/pm3/run</string>
<key>PM3_DATA_DIR</key>    <string>/Users/zhoufan/.local/share/pm3</string>
```

daemon 与 CLI 都走第 1 档优先级 ⇒ 值必然一致。**pm3 内部只认 `PM3_*`,`XDG_*` 仅在 startup 那一刻参与推导。**

这是 `pm3_env` 的**内容**变更不是新字段,但根 CLAUDE.md:72 的精神适用 ⇒ **三个 renderer 各加一条断言**(launchd 的 `<key>PM3_STATE_DIR</key>`、systemd 的 `Environment="..."`、schtasks 的 `.cmd` 里 `set`,注意 `%` 转义成 `%%`)。

`pm3_env` 填充点从 `run_service` 后移到 `open_service_session`(那里已有 config 和 paths);`ServiceContext` 保留该字段(测试要能注入),`service_tests.rs` 两处字面量跟改。

### A-6 `Pm3Config` 传播(6 处)

新增 `state_dir`/`runtime_dir`/`data_dir`,**每个必须 `#[serde(default)]`**(裸 default 即空串,语义正好是"替我推导";真机 config.yaml 不被升级重写,缺 default 会让新 daemon 解析不了自己的配置)。

6 处:仓内 `config.yaml`、`adapters/test_support/config_sections.rs:1`、`adapters/src/test_helpers/config_schema_test_helpers.rs:43`、`frameworks/test_support/config_fixtures.rs:39`+`:100`、`frameworks/tests/common/mod.rs:~206`、校验函数 + `every_error_variant` 表。

**校验放宽**:`validate_paths` 现在拒绝空 `home`/`cfg_dir`,必须改为允许空。但要保持 `InvalidHome`/`InvalidCfgDir` 两个变体的**分支可达**(否则 region 不可达)⇒ 语义改为"非空但既非绝对路径也不以 `~` 开头"时报。

### A-7 其他传播

- `SpecSource` 4 处(与 B 合并一次改):加 `apps_dir`/`state_dir`/`runtime_dir`/`data_dir`/`global_env`
- `file.rs:299 working_directory` 从 `home_dir` 改用 `apps_dir`。**cwd 进指纹 ⇒ 切 XDG 时全量 evict+respawn**(legacy 下 `apps_dir == home_dir` 一字节不变,代价只在切换那一刻付一次)
- `install/layout.rs:8 BACKUP_DIRECTORY` 与新 `paths.rs::BACKUPS_DIR` 是同一常量两份副本 ⇒ 删前者,`backup_root` 改收 data 根
- `ensure_layout` 创建并 chmod 四根 + logs + apps + cfg_dir。**chmod 失败只 warn 不传播**对四根都适用(复用 `restrict_to_owner`,不要加 `?`)
- `clear_runtime_files` **保持先删 pid 再删 socket**(反序让 `signal_semantics` e2e 25% 概率卡住)
- `install.sh:68` 与 `dev_scripts/monitor.ts:5-6` 要学会 XDG 路径;后者被 `rust_contract.test.ts:20-33` 用字符串路径守着 ⇒ **`just test-scripts` 必跑**

---

## B. 全局环境变量

### B-0 优先级只有两层,daemon 自身 env 不透传

```
子应用 env (cfg_dir/<name>.env 或 <name>.enc.yaml)
  > pm3 全局 env (config 根/pm3.env 或 pm3.enc.yaml)
    > pm3 注入的 HOME
```

**daemon 进程自己的 env 不透传给子进程**(`env_clear()` 保持)。想让服务拿到 `XDG_DATA_HOME`/`TZ`/`LANG`,就写进 `pm3.env`——显式、可备份、可审计。

不做白名单透传(`env_inherit: [...]`)的理由:全量继承会把 `PM3_SOPS_IDENTITY_FILE` 这类 pm3 内部变量泄给每个服务(含 danger-full-access 的);白名单则是第二份真相——同一个变量可能既在白名单里又在 `pm3.env` 里,优先级还要再定一层。写进 `pm3.env` 一处即可,YAGNI。

这正好解决 caddy 的问题:`pm3.env` 里写一条 `XDG_DATA_HOME=${HOME}/.local/share`,所有服务都拿到,不用逐个配。

### B-1 文件与解析:一行新解析器都不写

- 明文 `<config 根>/pm3.env`,密文 `<config 根>/pm3.enc.yaml`
- 复用 `load_env_file` / `enc_file_present` / `load_enc_file` / `parse_env_text` / `secure_file`(读后 chmod 0600 + `symlink_metadata` 排除符号链接)
- **密文走 `parse_env_text`(Verbatim)不是 `parse_env_file`(Shell)**:sops `--output-type dotenv` 已输出解析后的值,再剥引号/解 `\xNN`/trim 会静默损坏密钥
- 优先级 `pm3.enc.yaml` > `pm3.env`;`enc_file_present` 只有 NotFound 算不存在(悬空符号链接或 EACCES 落回明文会让服务带空环境启动,而 stat 一恢复指纹就翻)

### B-2 解析时机:daemon 启动一次,注入 `SpecSource`

不在 `resolve_environment` 里现读。三条理由:性能(19 个服务 × `sops_timeout_ms` 5s);**语义正好是你要的**(改全局 env 需重启 daemon,与 `sops_identity_file` 已有行为同构);不引入内部可变性(`SpecSource` 藏在 `Arc` 后,塞 `OnceCell` 会把 interior mutability 带进 adapter 层)。

落点 `daemon/service.rs`,**在 `ensure_layout` 之后、`bind_uds` 之前**(bind 之后失败会留下已绑定的 socket)。

失败 ⇒ `Err` ⇒ daemon 在 serve 之前退出,**一个服务都不驱逐**(与 per-app 侧车的 `blocks_takeover` 同一 fail-safe 方向,但更简单——还没开始 resurrect)。新增 `Error::GlobalEnvironment { path, reason }`,service manager 的重启使其成为自愈重试。

### B-3 三层合并

`source.rs:141 with_host_home` **删掉**,换成 `merge_environment(layers: &[&[EnvValue]])`——`BTreeMap` 折叠,后层覆盖前层。调用 `merge_environment(&[&injected_home, &self.global_env, &app_declared])`。

三条既有行为由 last-wins 自然保住:应用声明 `HOME` 时应用胜出;`host_home=None` 不注入;每层内部 key 升序。输出顺序从"HOME 打头+其余升序"变成"全部升序"——无影响(`tokio_launcher` 逐个 `command.env`,顺序无关;`render_identity` 自己排序)且更确定。

### B-4 全局值不进指纹(本块最关键)

全局值合进了 `spec.env` 而后者进指纹 ⇒ 不处理则改一次全局 env 会在下次 handover 驱逐重启每个服务,与决策正相反。

`fingerprint.rs:94 sorted_env` 加 `entries.retain(|e| e.scope != EnvScope::Global)`。`Injected`(HOME)**保留**(根 CLAUDE.md:180:HOME 是指纹一部分,安全性依赖 unit 导出安装时的 HOME),`App` 保留。

要钉住:`changing_a_global_value_leaves_the_identity_unchanged` / `changing_an_app_value_changes_the_identity`。

**写进 `usecases/CLAUDE.md` 的推论**:全局与应用同 key 时删掉应用那条 ⇒ 指纹少一 entry ⇒ digest 变 ⇒ 重启一次。这是正确行为(生效值确实换了来源)但会让人意外。

### B-5 sealed 可见性

全局 `.enc.yaml` 存在但无 identity ⇒ `Secrets::Sealed` ⇒ 全局层为空 + 复用 `log_unopened_secrets` 的 warn。

`list` 的 `env:sealed` 通告**不动**:它读 `view.env_origin`,语义是"这个应用自己的密文没打开",不该被全局状态污染。

---

## C. describe 脱敏展示

### C-1 脱敏在 daemon 侧,值离开 usecases 之前

明文塞进 DTO 会同时破三处:`describe --json` 直接打明文;被拒时 report 进 `resp` 日志(`log_level: debug` 下 token 落盘);两条守护断言当场变红(**它们红是对的**)。

```rust
// entities/src/process/secret.rs(新)
pub const ELIDED: &str = "..";
const VISIBLE_CHARS: usize = 4;
const MIN_MASKABLE_BYTES: usize = 16;
pub fn mask_secret(value: &str) -> String
```

放 entities 的理由:这是**业务安全不变量**("pm3 从不传输凭据明文"),不是排版。此条写进根 CLAUDE.md(它看起来像 presenter 的活)。

格式 `头4..尾4 <字节数>`,阈值 16 字节:

```
env CF_API_TOKEN   abcd..wxyz 40
env XDG_DATA_HOME  /Use..hare 28
env TZ             .. 14
env PORT           .. 4
```

- **`len() >= 16`(字节)** → 头 4 字符 + `..` + 尾 4 字符 + 空格 + 字节数
- **`len() < 16`** → `..` + 空格 + 字节数(不露任何字符)

**阈值必须是 16 不是 8**:长度 8 时头 4 + 尾 4 就是**整个值**,等于不脱敏;长度 9 只藏 1 个字符。16 保证至少藏住 8 个字符。常见密钥(token/密码)都 ≥ 16,更短的值本来就不该露字符。

**长度按 utf8 字节计**(CLAUDE.md 规则 20),而**截断按字符**——按字节切会切出半个码位。这两个口径不同是有意的,测试要钉住。

`..` 用 ASCII 而非 `…`:不受终端宽度歧义影响。

显示长度的价值:能判断"我的 40 字符 token 是不是被截断成 32 了"——这是原方案(固定 8 星、刻意不泄露长度)做不到的诊断能力。代价是泄露长度,但对 ≥ 16 字节的密钥而言长度不构成有效信息。

实现:clippy `string_slice` 禁一切 `&value[..n]` ⇒ 用 `chars().take(4)` 与 `chars().skip(count-4)`(**用 `skip` 不用两次 `rev`**,后者多一个闭包 region)。字节数用 `value.len()`。

### C-2 `ProcessView` 与 DTO

`ProcessView` 加 `env: Vec<EnvDisplay>`(value 已脱敏),`view()` 里 `mask_secret` 一次。`ProcessView` **保留** `env_declared: usize`(由 `view()` 从 `declared_env_count()` 填),DTO 的 `From` 保持逐字段拷贝。

**两条守护断言收紧而非删除**:`the_serialized_dto_has_no_env_field` → `the_serialized_dto_carries_no_cleartext_value`(断言不含明文**且**含脱敏形态);`list_resources.rs:41` 同样。

DTO 统一带 `env` + `skip_serializing_if = "Vec::is_empty"`,list 与 describe 都带(值已脱敏无泄露),换来"一个 DTO"的简单。

### C-3 排版

`describe.rs` 拆两函数,**不改那 17 行固定行的类型**。`render_env_block` 自己对齐(变量名可能 30+ 字符,参与 `widest` 会把固定行的值挤到右边):

```
env               encrypted (7 values)
env CF_API_TOKEN   abcd..wxyz 40  (app)
env XDG_DATA_HOME  /Use..hare 28  (global)
env HOME           /Use..ofan 14  (pm3)
env TZ             .. 14          (global)
```

`presenter_describe_tests.rs:4 value_of` 用 `strip_prefix` 顺序查找,固定 `env` 行排在 env 块之前 ⇒ 既有测试全绿。

---

## D. list 展示改进

从 101 字符压到约 53。

### D-1 列变更

| 列 | 现状 | 改为 |
|---|---|---|
| `id` | 从 0 起、不复用空缺(真机已跳到 34) | **从 1 起 + 复用已删除服务的空缺** |
| `name` | 16 | 不动(最关键的识别信息) |
| `pid` | 5 | **默认隐藏**,`--full` 才显示 |
| `status` | 7 | 不动(不缩写) |
| `↺` | `restart_time`(历史累计) | **`unstable_restarts`**(近期) |
| `uptime` | 3 | 不动 |
| `rss` + `cpu` | 两列 12 | **合并一列** `140.3M/0.0%`;整列全无采样值时隐藏 |
| `next` | `09:19+08:00`(11) | **`09:19`**,时区移到表头 `next(+8)` |
| `sandbox` | 22(最宽,18/19 行带 `+net`) | **三位标志位,恒定 3 字符**(下) |
| 通告列 | 仅 `env:sealed` | 加诊断标记(下) |
| 列间距 | 2 空格 | **1 空格** |

排序:**按 name 升序**(现在是 `ProcessTable.records` 的 Vec 顺序)。

`--full` 显示全部列(pid、cpu 独立列、完整 sandbox 文字)。

### D-1b sandbox 三位标志位

表头 `box`,位置固定,像 `ls -l` 那样竖着扫一眼就能看出哪个服务权限开得大:

```
位1 写: -只读(read-only)  W工作区(workspace-write)  F全放开(danger-full-access)
位2 读: -最小(minimal)    R全部(full)
位3 网: -无               N有
```

真机效果(19 行里 15 个 `W-N`、3 个 `F-N`、1 个 `--N`):

```
name              box
mihomo-rule       W-N
sshd              F-N
ssh-tunnel-r      --N
caddy             W-N
tailscaled        F-N
```

**读与写是正交的两个维度**——`SandboxPolicy.mode`(写)与 `.read`(读范围)是独立字段,`read-only` + `read: full`(什么都不能写但能读整个文件系统)是合法组合,所以不能把两维压进一个字母。位 2 在真机全是 `-`(`read: full` 出现 0 次),但保留它才能完整表达。

新增 `format_sandbox_flags(mode, read, network) -> String` 替换 `format_sandbox`(fields.rs:96)。`format_sandbox` 保留给 describe(那里要完整文字)。

**region 陷阱**:三个维度各自是 match,位 1 三分支 + 位 2 两分支 + 位 3 两分支 = 7 个 region,**每个都要单测**。不要写成 or-pattern(每个 alternative 独立计 region)。至少 7 个 case + 一个"全默认"与一个"全放开"的组合断言。

### D-2 通告列诊断标记

通告列(第 11 列,空表头)是既有设计——`join_row` 的 `trim_end` 保证无内容时**逐字节等同于该列不存在**,健康服务不加宽。按优先级只取一个:

| 标记 | 条件 |
|---|---|
| `breaker:553` | `Errored && max_restarts > 0 && unstable >= max_restarts` |
| `noselfheal` | `Errored && !autorestart`(声明了不自愈,errored 即终态) |
| `flapping:553` | `unstable > 0` 且未跳闸(`max_restarts: 0` 的情况全落这里,正好暴露那个陷阱) |
| `env:sealed` | 现有 |

多个用 `,` 连接。`HEADERS`/`COLUMNS` 常量**不变**(仍 11 列)。

### D-3 `unstable_restarts` 陈旧问题

直接换列,真机 mihomo-global 会显示 `↺ 17` 而它已健康 20 小时——比现在更误导。因为它只在退出时重算,没有"已经稳住了"的清零时刻。

修法(复用已有无条件 tick,不新增 JoinHandle 表):`usecases/src/query.rs` 加**非泛型纯函数** `settled_stability(record, now_ms) -> bool`——记录在跑、`elapsed >= min_uptime_ms`、`unstable > 0` ⇒ 清零。调用点 `on_liveness_sample` 循环之外(它已是无条件重排的 tick),清零后 `save_table`。

**必须是 `query.rs` 里的非泛型纯函数**:写在 `Supervisor::on_*` 里会因 `impl Ports` 泛型被 fake 与真实例各走一半,**再多测试也填不满 region**(同 `unswept_pids` 的抽取理由)。

语义与 `decide_restart:45` 本来就做的清零一致,只是发生得更早、更可见。

### D-4 `ProcessView` 传播

加 `unstable_restarts` / `max_restarts` / `autorestart`(通告列需要)。**`ProcessView` 是字段访问式构造,加字段不会编译报错**,只会静默漏掉 ⇒ `view()` 的填充必须手工核对。describe 加 `unstable restarts` 行(注意 `restart_circuit_breaker.rs:66` 用 `starts_with("restarts")`,新行以 `unstable` 开头不会误命中)。

---

## E. liveness 跳过 `autorestart: false`

`query.rs:181 liveness_watch_list` 加一条 `.filter(|record| record.spec.autorestart)`。

现状是**只杀不救**:liveness 连续失败达阈值 → `restart_now` → `request_stop`(发信号)+ `request_restart` → `hand_to_the_breaker` 置 supervised → 进程退出 → `settle_without_the_breaker` 返回 None → `decide_restart` → `!autorestart` → `GiveUp` → `Errored`。运维明确说了不要自动管,pm3 却把它弄死了,而且**现有测试完全没覆盖这个组合**。

1 行 + 1 个纯函数单测(`liveness_watch_list` 已是非泛型纯函数,天然好测)。真机立即影响为零(caddy 因 errored 本就不在名单),但配合业务侧"去掉 `autorestart:false`"之后这条决定才有实际后果。

新增测试:`a_service_that_declined_autorestart_is_not_watched_for_liveness`。

---

## 跨块公共改动(必须先做)

### 公共-1 `config/schema.rs` 已 515 行超限,先拆

已违反"代码文件 MUST < 512 行"。A 块要加 4 字段 + 4 default fn + 4 条校验,只会更糟。

新建 `adapters/src/config/validate.rs` 搬走 `validate_config` 及其全部下游;`schema.rs` 只留 struct + 常量 + `default_*` + `ConfigError`(**留在 schema.rs**,否则 `every_error_variant` 表的 import 全变)。测试跟着搬到 `validate.rs` 末尾的 `config_validate_tests.rs`(**文件名必须 `_tests.rs` 结尾**,否则覆盖率门禁当它是"生产文件缺失")。

**独立的第一个 commit**,纯搬迁零行为变更。**必跑 `just test-scripts`**。

### 公共-2 保留名 `config` 与 `pm3`

`entities/src/process/spec.rs`:新增 `RESERVED_FILE_NAMES: [&str; 2] = ["config", "pm3"]`(不要两个常量 + 两个 if,那是 DRY 违反且多一个 region),`validate_app_name` 在 `RESERVED_ALL_SELECTOR` 后插一条检查,**一个** `SpecError::ReservedFileName { name }` 变体(带字段的变体只有一个 region)。文案以 `cannot accept` 开头。

修掉那个 `--force` 覆写 daemon 配置的漏洞,可独立先合。

### 公共-3 环境变量携带来源

```rust
// entities/src/process/env.rs(新)
pub enum EnvScope { Injected, Global, App }
pub struct EnvValue { key, value: String, scope: EnvScope }
```

`AppSpec.env`: `Vec<(String,String)>` → `Vec<EnvValue>`。**不用平行数组**(两个 Vec 会失同步,而 `AppSpec` 的两个静默吞字段站点不会替你发现)。

**指纹逐字节不变的论证**(必须写进 commit message):`sorted_env` 仍按 `(key,value)` 排序忽略 scope;`entry_line` 仍输出 `key=value` 不含 scope ⇒ 输出逐字节不变 ⇒ `launch_digest` 不变 ⇒ 真机 19 个服务照常 adopt。

`AppSpec.env_declared` **删除**,改为 `declared_env_count()` 方法(按 scope 过滤)。净收益:字段 20→19;`fingerprint.rs:27` 那行 `env_declared: _` 消失;`source.rs:79` 的"必须在合并 HOME 之前取 len"时序陷阱从此不可能踩错。

改完先跑 `cargo build --workspace --all-targets --offline` 让 E0063/E0560 点名。**唯一编译失败的生产文件是 `fingerprint.rs:20-41` 的全字段解构**;另两个静默站点(`persistence/dto.rs` 的 `encode_state`、`record.rs:49` 的 `view`)**必须手工检查**。

`LaunchSpec.env` 保持 `Vec<(String,String)>`——scope 是 pm3 内部账本,launcher 不该知道。`build_launch_spec` 从零加工变成 `.map(|e| (e.key.clone(), e.value.clone()))`,这是**唯一**允许的加工点且必须在 usecases 侧。

---

## 覆盖率 region 陷阱清单

- `mask_secret` 两分支;`EnvScope::as_str()` 三个 variant 各一测。`mask_secret` 至少 5 条:≥16 露头尾、恰好 16(边界)、15(另一侧边界)、空串、**多字节值**(中文/emoji,证明截断按字符而长度按字节)
- `render_env_block` 的 `if list.is_empty()` **闭合 `}` 有独立 region** ⇒ 写成早返回 `if list.is_empty() { return String::new(); }`
- 新 error 变体(`GlobalEnvironment`/`SocketTooLong`/`ReservedFileName`)各构造 + `to_string()` 断言一次
- `EncFileError::path()` 是五分支 or-pattern 合一,**每个 alternative 独立 region** ⇒ 不新增变体(复用现有五个)
- `tracing::debug!(field = <expr>)` 的表达式只在有 subscriber 时求值 ⇒ `log_environment` 多记全局条数必须 `let global = ...;` 先算
- runtime 根的 `probe` 参数让两个分支在两平台都可达(否则 macOS 那条永不可达)
- 新增 `xdg_layout.rs` e2e 会给 `frameworks` 再加一份 instantiation——**本轮最可能踩的覆盖率坑**
- `presenter_table_tests.rs` 的 fixture 里 `restart_time` 与 `unstable_restarts` **必须取不同值**,否则测试通过也证明不了 `↺` 换了数据源

## 测试先行

顺序:公共(全红)→ A → B → C → D。命名是完整英文句子,断言必带消息且回显实际值。

关键几条:
- `a_service_cannot_be_named_config_because_it_would_overwrite_the_daemon_config`
- `the_runtime_root_falls_back_to_the_state_directory_when_run_user_does_not_exist` / `..._uses_run_user_when_the_kernel_provides_it`(probe 两分支)
- `the_single_root_layout_keeps_every_path_where_it_was`(legacy 回归,逐字段对齐现有 `paths_tests.rs:5-13`)
- `the_working_directory_stays_writable_when_the_state_root_is_hidden_above_it`(PM3-44 形状:断言 deny 在 allow 之前且 cwd 的 allow **没有** `require-not`)
- `changing_a_global_value_leaves_the_identity_unchanged` / `changing_an_app_value_changes_the_identity` / `the_injected_home_stays_part_of_the_identity`
- `an_app_value_wins_over_a_global_value_with_the_same_key` / `a_global_value_wins_over_the_injected_home`
- `describe_shows_only_the_first_and_last_four_characters_of_a_long_value` / `describe_hides_every_character_of_a_short_value` + 多字节值一条(截断按字符、长度按字节)
- e2e `a_daemon_started_with_the_split_layout_serves_the_cli`(证明 CLI 与 daemon 算出同一个 socket,最重要的一条)
- e2e `an_unreadable_global_environment_keeps_the_daemon_from_taking_over`(断言 daemon 退出、**没有服务被驱逐**)

## 验证

```
cargo +nightly fmt --all
just lint            # 必须在 cov 之前(cov 只跑 nextest,看不到过期 #[expect])
just cov --fresh     # 行号大幅移动后必须 fresh
just test-scripts    # 公共-1 动了 adapters 布局,rust_contract 守卫必跑
just check-windows   # A 块动路径,命名管道/marker 文件要能编译
just typecheck       # 改了 monitor.ts
```

手工验证用 `mktemp -d`(**不要用 scratchpad 路径**,太长撞 macOS `SUN_LEN`):

```
d=$(mktemp -d)
env -u PM3_HOME PM3_CONFIG_DIR="$d/cfg" PM3_STATE_DIR="$d/state" \
    PM3_RUNTIME_DIR="$d/run" PM3_DATA_DIR="$d/data" \
    target/release/pm3 config show
```

真机验证**只走日志与状态**(`.env` 字面串被 hook 拦;cfg_dir 下凭据文件连 Read 都被拒;"读那个文件"不能出现在任何验证步骤):

```
rg '"action":"load_env"' <state>/pm3.log | tail -20      # 全局条数是否并入
rg '"action":"decrypt_env"' <state>/pm3.log | rg pm3.enc  # 全局密文解开
pm3 describe caddy | rg '^env '                           # 每行形如 `env K  abcd…wxyz  (app)`,出现完整值即失败
pm3 describe caddy --json | rg -c '…|\*\*\*\*'
ls -ld <config> <state> <data> <state>/run                # 都应 drwx------
nc -z -U <state>/run/pm3.sock; echo $?                    # 沙箱内必须 1,宿主 0
```

## 真机迁移

**升级零中断;切 XDG 有几秒窗口,不可避免**(cwd 进指纹 ⇒ 必然全量 evict+respawn;`evict_all` 并行,窗口 ≈ 一个 `kill_timeout_ms` 1600ms + spawn)。

**第一阶段(零中断,可立即做)**:`just install`。真机 config.yaml 不被重写 → legacy → 19 个服务 adopt。验收:`pm3 list` 19 行;install 的 before/after diff 报告 `lost` 为空;`launchctl list | rg com.enjoypi.pm3` 的 PID 列不是 `-`;`rg '"action":"resurrect"' pm3.log` 无 evict/respawn。

**第二阶段(择时)**:4 个 danger-full-access 服务会被 SIGTERM + 重启,sshd 重启会断开活跃 ssh 会话 ⇒ **不要通过被管的 sshd 登录做这次迁移**。

```
1. pm3 unstartup                  # 先摘 launchd,否则它抢 socket
2. pm3 shutdown                   # 只停 daemon,19 个服务继续跑
3. mkdir -p ~/.local/state/pm3 ~/.local/share/pm3
   mv ~/.pm3/{dump.yaml,logs,pm3.log} ~/.local/state/pm3/
   mv ~/.pm3/install-backups ~/.local/share/pm3/
   chmod 700 ~/.local/state/pm3 ~/.local/share/pm3
4. mv ~/.pm3/config.yaml ~/.config/pm3/config.yaml
   # 编辑:home: ""  cfg_dir: ""  加三个空值键
5. ~/.local/bin/pm3 --config ~/.config/pm3/config.yaml startup --force
   # 必须用最终位置的二进制(否则 plist 钉到 target/release)
6. 等 "launchd 报的 pid == pm3.pid 内容" 成立后才跑第一条 CLI 命令
```

**旧的 `~/.pm3/<name>/` 工作目录不要删**(`mihomo-global/`、`probe/` 里有服务自己的运行数据),需要时逐个 `mv` 到 `~/.local/state/pm3/apps/<name>`。

**回滚**:config.yaml 移回 + 恢复 `home` 值 + `mv` 回 dump/logs + `startup --force`。legacy 代码路径一直在,不需换二进制。

## 实施顺序(6 个 commit,每个都过 lint + cov)

| # | 内容 | 依赖 | 风险 |
|---|---|---|---|
| 1 | `refactor: 拆分 config schema 与校验` | — | 零行为变更 |
| 2 | `feat: 保留 config 与 pm3 服务名` | — | 小;修掉覆写漏洞,可先合 |
| 3 | `refactor: 环境变量携带来源` | 1 | **最大**;`AppSpec.env` 类型变更 + 删 `env_declared`。行为零变更,但**唯一可能意外改指纹从而全量重启**的一步,验收标准就是那三条 fingerprint 测试 |
| 4 | `feat: XDG 布局` | 1,2 | legacy 保证真机零影响 |
| 5 | `feat: 全局环境变量` | 3,4 | 含全局值不进指纹的过滤 |
| 6 | `feat: describe 脱敏环境与 list 精简` | 3,5 | C + D + E 合一(都动 `ProcessView`/DTO/presenter,分开会两次改同一批传播点) |

上线节奏:6 个 commit 合完 → `just install`(零中断)→ 观察 24 小时(看 `↺` 有无异常跳动、`load_env` 条数对不对)→ 择时切 XDG。

## 明确不做

`liveness_exec`(pm3 **不支持**,`validate_liveness_probe` 明确拒绝周期性 exec 探针);全局 sealed 推到 list;脱敏字符数做成配置项;`pm3 env set/get`(pm3 只读不写 env);把 cron/手动 restart 计入 `restart_time`(会改断路器语义,改成文档记录);改 `max_restarts: 0` 为 pm2 语义(pm2 的 `0>=0` 首崩即弃几乎是无意 bug,pm3 的"无限制"更有用);拆 `Pm3Paths` 成多个 struct;macOS 用原生 `~/Library/...`;`pm3 migrate` 子命令(一次性操作写文档即可);失败通知/webhook;颜色。

## 真机业务侧修复(配置调整,不含在代码改动内)

- **caddy**:`caddy.env` 加 `XDG_DATA_HOME=${HOME}/.local/share` 修沙箱拒写;**去掉 `autorestart: false`、`max_restarts` 从 0 改成有限值(如 5)**——这样它崩溃会重启、连崩 5 次会 errored 停下,而不是像 09-15 那样循环 1419 次,liveness 也会开始工作
- **tunnel-health**:ENOENT 已在 09-13 修好(脚本已用绝对路径),`errored` 是历史遗留 ⇒ `pm3 reset tunnel-health`。它的 ssh 分支可退休(ssh 自带 `ServerAliveInterval` + `ExitOnForwardFailure`,断了自己退出、pm3 重启它);**远端探测无法被 pm3 替代**,建议保留但去掉 `pm3 restart` 动作改为只告警(两套重启逻辑叠加正是那 152 次无记账重启的来源)
- **ssh-tunnel-r**:caddy 修好自然恢复
