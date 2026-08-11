import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { arch, cpus, platform, release, tmpdir } from "node:os";
import { join } from "node:path";

const serviceCount = 8;
const hotSamples = 50;
const sleeperMs = 3_600_000;
const onlineDeadlineMs = 15_000;
const socketGoneDeadlineMs = 10_000;
const pollIntervalMs = 50;

export interface Summary {
  count: number;
  mean: number;
  median: number;
  p95: number;
}

interface ServiceView {
  name: string;
  status: string;
  pid: number | null;
}

export interface BenchReport {
  pm3Version: string;
  machine: string;
  dateUtc: string;
  coldStartMs: number;
  reclaimStartMs: number;
  idleRssKib: number;
  loadedRssKib: number;
  perServiceOverheadKib: number;
  childrenRssKib: number;
  startMs: Summary;
  listMs: Summary;
}

export function parseVmRssKib(status: string): number | null {
  const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status);
  const raw = match?.[1];
  if (raw === undefined) {
    return null;
  }
  const value = Number.parseInt(raw, 10);
  return Number.isNaN(value) ? null : value;
}

export function summarize(samples: number[]): Summary {
  if (samples.length === 0) {
    throw new Error("summarize needs at least one sample");
  }
  const sorted = [...samples].sort((a, b) => a - b);
  const total = sorted.reduce((acc, value) => acc + value, 0);
  const median = sorted.at(Math.floor((sorted.length - 1) / 2)) ?? 0;
  const p95 = sorted.at(Math.max(0, Math.ceil(sorted.length * 0.95) - 1)) ?? 0;
  return {
    count: sorted.length,
    mean: Math.round(total / sorted.length),
    median: Math.round(median),
    p95: Math.round(p95),
  };
}

export function parseViews(text: string): ServiceView[] {
  const decoded: unknown = JSON.parse(text);
  if (!Array.isArray(decoded)) {
    throw new Error("pm3 list --json did not answer an array");
  }
  return decoded.map((entry: unknown) => {
    if (typeof entry !== "object" || entry === null) {
      throw new Error("view entry is not an object");
    }
    const record = entry as Record<string, unknown>;
    const name = record["name"];
    const status = record["status"];
    const pid = record["pid"];
    if (typeof name !== "string" || typeof status !== "string") {
      throw new Error("view entry misses name/status");
    }
    return { name, status, pid: typeof pid === "number" ? pid : null };
  });
}

function mib(kib: number): string {
  return (kib / 1024).toFixed(1);
}

export function renderReport(report: BenchReport): string {
  const lines = [
    "| 指标 | 数值 |",
    "|---|---|",
    `| 冷启动（含拉起 daemon） | ${report.coldStartMs} ms |`,
    `| 冷启动（接管 ${serviceCount} 个在跑服务） | ${report.reclaimStartMs} ms |`,
    `| pm3 list 热路径（n=${report.listMs.count}） | mean ${report.listMs.mean} ms / median ${report.listMs.median} ms / p95 ${report.listMs.p95} ms |`,
    `| start 到 Online（n=${report.startMs.count}） | mean ${report.startMs.mean} ms / median ${report.startMs.median} ms / p95 ${report.startMs.p95} ms |`,
    `| daemon 空载 RSS | ${mib(report.idleRssKib)} MiB |`,
    `| daemon 带 ${serviceCount} 服务 RSS | ${mib(report.loadedRssKib)} MiB（每服务开销 ${report.perServiceOverheadKib} KiB） |`,
    `| ${serviceCount} 个被托管进程自身 RSS 合计 | ${mib(report.childrenRssKib)} MiB |`,
    "",
    `实测环境：${report.machine}；${report.pm3Version}；${report.dateUtc}；\`just bench\` 可重跑。`,
  ];
  return `${lines.join("\n")}\n`;
}

export function configYaml(home: string): string {
  return `pm3:
  home: "${home}"
  cfg_dir: "${home}/service"
  search_path: "/usr/bin:/bin:/opt/homebrew/bin"
  stop_signal: "TERM"
  kill_timeout_ms: 400
  start_timeout_ms: 15000
  drain_timeout_secs: 2
  request_timeout_ms: 30000
  command_timeout_ms: 5000
  daemon_poll_interval_ms: 40
  daemon_poll_max_interval_ms: 200
  memory_poll_interval_ms: 30000
  log_follow_interval_ms: 200
  log_tail_lines: 20
  log_read_max_bytes: 4194304
  log_rotate_max_bytes: 0
  log_rotate_interval_ms: 60000
  ready_timeout_ms: 30000
  ready_poll_interval_ms: 200
  daemon_channel_depth: 32
  request_body_limit_bytes: 131072
  restart:
    autorestart: true
    min_uptime_ms: 1000
    max_restarts: 15
    restart_delay_ms: 0
    max_restart_delay_ms: 15000
  sandbox:
    mode: "workspace-write"
    read: "minimal"
    network: false
    seatbelt_program: "/usr/bin/sandbox-exec"
    bwrap_program: "bwrap"
    minimal_read_roots:
      - "/bin"
      - "/sbin"
      - "/usr"
      - "/etc"
      - "/lib"
      - "/lib64"
      - "/opt/homebrew"
    forbidden_writable_roots:
      - "/"
      - "/etc"
      - "/usr"
  service:
    label: "pm3-fixture"
    restart_delay_secs: 2
    restart_condition: "always"
    max_tasks: 4096
    cpu_quota_percent: 0
    wait_for_network: false
    launchctl_path: "/bin/launchctl"
    systemctl_path: "/usr/bin/systemctl"
    loginctl_path: "/usr/bin/loginctl"
    schtasks_path: "schtasks"
    taskkill_path: "taskkill"

telemetry:
  service_name: "pm3"
  log_level: "info"
  log_format: "json"
`;
}

interface RunOutcome {
  stdout: string;
  stderr: string;
  code: number;
  ms: number;
}

async function run(command: string[]): Promise<RunOutcome> {
  const started = performance.now();
  const child = Bun.spawn([...command], { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(child.stdout).text();
  const stderr = await new Response(child.stderr).text();
  const code = await child.exited;
  return { stdout, stderr, code, ms: Math.round(performance.now() - started) };
}

async function pm3(bin: string, config: string, args: string[]): Promise<RunOutcome> {
  const outcome = await run([bin, "--config", config, ...args]);
  if (outcome.code !== 0) {
    throw new Error(`pm3 ${args.join(" ")} 退出码 ${outcome.code}: ${outcome.stderr.trim()}`);
  }
  return outcome;
}

async function daemonRssKib(home: string): Promise<number> {
  const pidText = await readFile(join(home, "pm3.pid"), "utf8");
  return rssKibOf(Number.parseInt(pidText.trim(), 10));
}

async function rssKibOf(pid: number): Promise<number> {
  const probe = await Bun.file(`/proc/${pid}/status`).text().catch(() => null);
  if (probe !== null) {
    const kib = parseVmRssKib(probe);
    if (kib !== null) {
      return kib;
    }
  }
  const ps = await run(["/bin/ps", "-o", "rss=", "-p", String(pid)]);
  const kib = Number.parseInt(ps.stdout.trim(), 10);
  if (ps.code !== 0 || Number.isNaN(kib)) {
    throw new Error(`读不到 pid ${pid} 的 RSS`);
  }
  return kib;
}

async function waitOnline(bin: string, config: string, name: string): Promise<void> {
  const deadline = performance.now() + onlineDeadlineMs;
  for (;;) {
    const listed = await pm3(bin, config, ["list", "--json"]);
    const view = parseViews(listed.stdout).find((entry) => entry.name === name);
    if (view?.status === "online") {
      return;
    }
    if (performance.now() > deadline) {
      throw new Error(`${name} 在 ${onlineDeadlineMs} ms 内没到 online（当前 ${view?.status ?? "缺失"}）`);
    }
    await Bun.sleep(pollIntervalMs);
  }
}

async function waitSocketGone(home: string): Promise<void> {
  const socket = join(home, "pm3.sock");
  const deadline = performance.now() + socketGoneDeadlineMs;
  for (;;) {
    if (!(await Bun.file(socket).exists())) {
      return;
    }
    if (performance.now() > deadline) {
      throw new Error("daemon 收尾超时：pm3.sock 仍在");
    }
    await Bun.sleep(pollIntervalMs);
  }
}

async function startServices(bin: string, config: string): Promise<number[]> {
  const samples: number[] = [];
  for (let index = 0; index < serviceCount; index += 1) {
    const name = `s${index}`;
    const started = performance.now();
    await pm3(bin, config, ["start", "--name", name, bin, "__sleep", String(sleeperMs)]);
    await waitOnline(bin, config, name);
    samples.push(Math.round(performance.now() - started));
  }
  return samples;
}

async function childrenRssKib(bin: string, config: string): Promise<number> {
  const listed = await pm3(bin, config, ["list", "--json"]);
  const pids = parseViews(listed.stdout).flatMap((view) => (view.pid === null ? [] : [view.pid]));
  let total = 0;
  for (const pid of pids) {
    total += await rssKibOf(pid);
  }
  return total;
}

async function hotListSamples(bin: string, config: string): Promise<number[]> {
  const samples: number[] = [];
  for (let index = 0; index < hotSamples; index += 1) {
    const listed = await pm3(bin, config, ["list"]);
    samples.push(listed.ms);
  }
  return samples;
}

async function ensureSandboxBackend(): Promise<void> {
  if (platform() !== "linux") {
    return;
  }
  const probe = await run(["/bin/sh", "-c", "command -v bwrap"]);
  if (probe.code !== 0) {
    throw new Error("bench 需要 bwrap（默认沙箱后端）；请先安装 bubblewrap");
  }
}

async function collect(): Promise<BenchReport> {
  await ensureSandboxBackend();
  const build = Bun.spawn(["cargo", "build", "--locked", "--release", "-p", "frameworks"], {
    stdout: "inherit",
    stderr: "inherit",
  });
  if ((await build.exited) !== 0) {
    throw new Error("cargo build 失败");
  }
  const home = await mkdtemp(join(tmpdir(), "pm3-bench-"));
  const bin = join(process.cwd(), "target", "release", "pm3");
  const config = join(home, "config.yaml");
  try {
    await writeFile(config, configYaml(home), { mode: 0o600 });
    const version = (await pm3(bin, config, ["--version"])).stdout.trim();
    const cold = await pm3(bin, config, ["list"]);
    const idleRssKib = await daemonRssKib(home);
    const startSamples = await startServices(bin, config);
    const loadedRssKib = await daemonRssKib(home);
    const children = await childrenRssKib(bin, config);
    const listSamples = await hotListSamples(bin, config);
    await pm3(bin, config, ["kill"]);
    await waitSocketGone(home);
    const reclaim = await pm3(bin, config, ["list"]);
    return {
      pm3Version: version,
      machine: `${platform()} ${release()} ${arch()}, ${cpus().length} 核`,
      dateUtc: new Date().toISOString(),
      coldStartMs: cold.ms,
      reclaimStartMs: reclaim.ms,
      idleRssKib,
      loadedRssKib,
      perServiceOverheadKib: Math.round((loadedRssKib - idleRssKib) / serviceCount),
      childrenRssKib: children,
      startMs: summarize(startSamples),
      listMs: summarize(listSamples),
    };
  } finally {
    await pm3(bin, config, ["kill", "--with-services"]).catch(() => undefined);
    await waitSocketGone(home).catch(() => undefined);
    await rm(home, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  const report = await collect();
  process.stdout.write(renderReport(report));
}
