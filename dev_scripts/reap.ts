import { rm } from "node:fs/promises";
import { dirname, join } from "node:path";

const fixtureMarkers = ["pm3-e2e-never-installed", "pm3-fixture"];
const maxSignalablePid = 2_147_483_647;
const termGraceMs = 2000;
const pollIntervalMs = 100;

export interface ProcessRow {
  pid: number;
  ppid: number;
  args: string;
}

export interface DaemonArgs {
  exe: string;
  configPath: string;
}

export interface DaemonRef {
  pid: number;
  configPath: string;
}

export interface ReapPlan {
  daemons: DaemonRef[];
  servicePids: number[];
  dirs: string[];
}

export function parseProcessTable(text: string): ProcessRow[] {
  const rows: ProcessRow[] = [];
  for (const line of text.split("\n")) {
    const match = /^\s*(\d+)\s+(\d+)\s+(\S.*)$/u.exec(line);
    const pid = Number(match?.[1]);
    const ppid = Number(match?.[2]);
    const args = match?.[3];
    if (args === undefined || Number.isNaN(pid) || Number.isNaN(ppid)) {
      continue;
    }
    rows.push({ pid, ppid, args });
  }
  return rows;
}

export function parseDaemonArgs(args: string): DaemonArgs | null {
  const tokens = args.split(/\s+/u);
  const [exe, subcommand, flag, ...rest] = tokens;
  if (exe === undefined || !exe.endsWith("/pm3")) {
    return null;
  }
  if (subcommand !== "daemon" || flag !== "--config" || rest.length === 0) {
    return null;
  }
  return { exe, configPath: rest.join(" ") };
}

export function hasFixtureMarker(configText: string): boolean {
  return fixtureMarkers.some((marker) => configText.includes(marker));
}

function isSignalable(pid: number): boolean {
  return pid >= 2 && pid <= maxSignalablePid;
}

export function descendantsOf(
  rows: readonly ProcessRow[],
  rootPid: number,
): number[] {
  const childrenByParent = new Map<number, number[]>();
  for (const row of rows) {
    const siblings = childrenByParent.get(row.ppid) ?? [];
    siblings.push(row.pid);
    childrenByParent.set(row.ppid, siblings);
  }
  const found: number[] = [];
  const queue: number[] = [rootPid];
  let parent = queue.shift();
  while (parent !== undefined) {
    for (const child of childrenByParent.get(parent) ?? []) {
      if (!isSignalable(child) || found.includes(child)) {
        continue;
      }
      found.push(child);
      queue.push(child);
    }
    parent = queue.shift();
  }
  return found;
}

export function planReap(
  rows: readonly ProcessRow[],
  targetRoot: string,
  isFixture: (configPath: string) => boolean,
): ReapPlan {
  const daemons: DaemonRef[] = [];
  const servicePids: number[] = [];
  const dirs = new Set<string>();
  for (const row of rows) {
    const parsed = parseDaemonArgs(row.args);
    if (parsed === null) {
      continue;
    }
    if (row.ppid !== 1 || !isSignalable(row.pid)) {
      continue;
    }
    if (!parsed.exe.startsWith(`${targetRoot}/`)) {
      continue;
    }
    if (!isFixture(parsed.configPath)) {
      continue;
    }
    daemons.push({ pid: row.pid, configPath: parsed.configPath });
    servicePids.push(...descendantsOf(rows, row.pid));
    dirs.add(dirname(parsed.configPath));
  }
  return { daemons, servicePids, dirs: [...dirs] };
}

async function listProcessRows(): Promise<ProcessRow[]> {
  const ps = Bun.spawn(["ps", "-eo", "pid=,ppid=,args="], {
    stderr: "ignore",
    stdout: "pipe",
  });
  const text = await new Response(ps.stdout).text();
  await ps.exited;
  return parseProcessTable(text);
}

async function configIsFixture(configPath: string): Promise<boolean> {
  const file = Bun.file(configPath);
  if (!(await file.exists())) {
    return true;
  }
  return hasFixtureMarker(await file.text());
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function signalAll(pids: readonly number[], signal: "SIGTERM" | "SIGKILL"): void {
  for (const pid of pids) {
    if (!isSignalable(pid)) {
      continue;
    }
    try {
      process.kill(pid, signal);
    } catch {
      continue;
    }
  }
}

async function waitForExit(
  pids: readonly number[],
  graceMs: number,
): Promise<void> {
  const deadline = Date.now() + graceMs;
  while (Date.now() < deadline && pids.some(alive)) {
    await Bun.sleep(pollIntervalMs);
  }
}

export async function reapOrphanedDaemons(
  targetRoot: string,
): Promise<ReapPlan> {
  const rows = await listProcessRows();
  const fixtureByPath = new Map<string, boolean>();
  for (const row of rows) {
    const parsed = parseDaemonArgs(row.args);
    if (parsed === null || fixtureByPath.has(parsed.configPath)) {
      continue;
    }
    fixtureByPath.set(parsed.configPath, await configIsFixture(parsed.configPath));
  }
  const plan = planReap(
    rows,
    targetRoot,
    (path) => fixtureByPath.get(path) ?? false,
  );
  const victims = [
    ...plan.daemons.map((daemon) => daemon.pid),
    ...plan.servicePids,
  ];
  signalAll(victims, "SIGTERM");
  await waitForExit(victims, termGraceMs);
  signalAll(victims, "SIGKILL");
  for (const dir of plan.dirs) {
    await rm(dir, { force: true, recursive: true });
  }
  return plan;
}

if (import.meta.main) {
  const plan = await reapOrphanedDaemons(join(process.cwd(), "target"));
  for (const daemon of plan.daemons) {
    process.stdout.write(
      `reaped e2e daemon pid=${daemon.pid} ${daemon.configPath}\n`,
    );
  }
}
