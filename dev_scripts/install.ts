import { copyFile, mkdir, rename } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

import { runCargo } from "./cargo_invocation.ts";
import {
  backupRoot,
  backupStamp,
  compareServices,
  describeComparison,
  parseListedServices,
  parseServiceReport,
  parseLaunchdPid,
  parseMainPid,
  parsePidFile,
  parseWriteTargets,
  runtimeDirectory,
  type ServiceReport,
  type ServiceRow,
} from "./install_plan.ts";

const releaseBinary = "target/release/pm3";
const configSource = "config.yaml";
const stagingSuffix = ".incoming";
const optimisedRelease = { CARGO_PROFILE_RELEASE_OPT_LEVEL: "3" };
const destinationVariable = "PM3_INSTALL_PATH";
const backupVariable = "PM3_INSTALL_BACKUPS";
const dryRunFlag = "--dry-run";
const exitPolls = 200;
const supervisionPolls = 100;
const exitIntervalMs = 50;
const runningStatus = "running";
const launchdKind = "launchd";
const systemdKind = "systemd";
const systemctlProgram = "/usr/bin/systemctl";
const mainPidProperty = "MainPID";
const homeVariable = "PM3_HOME";
const runtimeDirVariable = "XDG_RUNTIME_DIR";
const defaultRuntimeHome = ".pm3";
const pidFileName = "pm3.pid";

interface Captured {
  code: number;
  output: string;
}

interface Installation {
  destination: string;
  backups: string;
  source: string;
}

function homeDirectory(): string {
  const home = Bun.env["HOME"];
  if (home === undefined || home.length === 0) {
    throw new Error("cannot install pm3: no HOME in the environment");
  }
  return home;
}

function runtimeHome(): string {
  return Bun.env[homeVariable] ?? join(homeDirectory(), defaultRuntimeHome);
}

function installation(): Installation {
  const home = homeDirectory();
  return {
    destination: Bun.env[destinationVariable] ?? join(home, "bin", "pm3"),
    backups: backupRoot(Bun.env[backupVariable], runtimeHome()),
    source: configSource,
  };
}

function userScopeEnvironment(): Record<string, string | undefined> {
  const uid = process.getuid?.() ?? 0;
  return {
    ...Bun.env,
    [runtimeDirVariable]: runtimeDirectory(Bun.env[runtimeDirVariable], uid),
  };
}

async function capture(
  command: readonly string[],
  env: Record<string, string | undefined> = Bun.env,
): Promise<Captured> {
  const spawned = Bun.spawn([...command], { stderr: "pipe", stdout: "pipe", env });
  const [stdout, stderr, code] = await Promise.all([
    new Response(spawned.stdout).text(),
    new Response(spawned.stderr).text(),
    spawned.exited,
  ]);
  return { code, output: `${stdout}${stderr}` };
}

async function announce(step: string, detail: string): Promise<void> {
  await Bun.write(Bun.stdout, `pm3 install: ${step}\n${detail}\n`);
}

async function buildOptimisedRelease(): Promise<number> {
  return runCargo(
    ["build", "-p", "frameworks", "--release", "--locked"],
    optimisedRelease,
  );
}

export async function listedServices(
  binary: string,
  source: string,
): Promise<ServiceRow[]> {
  if (!(await Bun.file(binary).exists())) {
    return [];
  }
  const listed = await capture([binary, "--config", source, "list"]);
  if (listed.code !== 0) {
    throw new Error(`cannot list the running pm3 services:\n${listed.output}`);
  }
  return parseListedServices(listed.output);
}

async function backUp(paths: readonly string[], into: string): Promise<void> {
  await mkdir(into, { recursive: true, mode: 0o700 });
  for (const path of paths) {
    if (await Bun.file(path).exists()) {
      await copyFile(path, join(into, basename(path)));
    }
  }
}

async function replaceBinary(destination: string): Promise<void> {
  await mkdir(dirname(destination), { recursive: true });
  const staged = `${destination}${stagingSuffix}`;
  await copyFile(releaseBinary, staged);
  await rename(staged, destination);
}

export async function overwrittenByInstall(
  binary: string,
  source: string,
): Promise<string[]> {
  if (!(await Bun.file(binary).exists())) {
    return [];
  }
  const planned = await capture([
    binary,
    "--config",
    source,
    "service",
    "install",
    dryRunFlag,
  ]);
  if (planned.code !== 0) {
    throw new Error(`cannot plan the pm3 service install:\n${planned.output}`);
  }
  return parseWriteTargets(planned.output);
}

async function daemonIsRunning(binary: string): Promise<boolean> {
  const found = await capture(["pgrep", "-f", `${binary} daemon`]);
  return found.code === 0;
}

async function waitForDaemonExit(binary: string): Promise<void> {
  for (let poll = 0; poll < exitPolls; poll += 1) {
    if (!(await daemonIsRunning(binary))) {
      return;
    }
    await Bun.sleep(exitIntervalMs);
  }
  throw new Error(
    `cannot reclaim services: the previous '${binary} daemon' never left`,
  );
}

async function reinstallService(
  binary: string,
  source: string,
): Promise<string> {
  const removed = await capture([
    binary,
    "--config",
    source,
    "service",
    "uninstall",
  ]);
  if (removed.code !== 0) {
    throw new Error(`cannot stop the running pm3 service:\n${removed.output}`);
  }
  const stopped = await capture([binary, "--config", source, "kill"]);
  if (stopped.code !== 0) {
    throw new Error(`cannot stop the running pm3 daemon:\n${stopped.output}`);
  }
  await waitForDaemonExit(binary);
  const installed = await capture([
    binary,
    "--config",
    source,
    "service",
    "install",
    "--force",
  ]);
  if (installed.code !== 0) {
    throw new Error(`cannot install the pm3 service:\n${installed.output}`);
  }
  return [removed.output, stopped.output, installed.output]
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .join("\n");
}

export async function readServiceReport(
  binary: string,
  source: string,
): Promise<ServiceReport> {
  const reported = await capture([binary, "--config", source, "service"]);
  const report = parseServiceReport(reported.output);
  if (report === undefined) {
    throw new Error(`cannot read the pm3 service status:\n${reported.output}`);
  }
  return report;
}

async function servingPid(): Promise<number | undefined> {
  const file = Bun.file(join(runtimeHome(), pidFileName));
  if (!(await file.exists())) {
    return undefined;
  }
  return parsePidFile(await file.text());
}

export async function systemdMainPid(
  systemctl: string,
  unit: string,
): Promise<number | undefined> {
  const shown = await capture(
    [systemctl, "--user", "show", "-p", mainPidProperty, "--value", unit],
    userScopeEnvironment(),
  );
  if (shown.code !== 0) {
    return undefined;
  }
  return parseMainPid(shown.output);
}

export async function supervisedPid(
  report: ServiceReport,
  systemctl: string = systemctlProgram,
): Promise<number | undefined> {
  if (report.kind === launchdKind) {
    const listed = await capture(["/bin/launchctl", "list", report.label]);
    return parseLaunchdPid(listed.output);
  }
  if (report.kind === systemdKind) {
    return systemdMainPid(systemctl, basename(report.unitPath));
  }
  return servingPid();
}

export async function waitForSupervision(
  binary: string,
  source: string,
  systemctl: string = systemctlProgram,
): Promise<ServiceReport | undefined> {
  for (let poll = 0; poll < supervisionPolls; poll += 1) {
    try {
      const report = await readServiceReport(binary, source);
      const supervised = await supervisedPid(report, systemctl);
      if (
        report.status === runningStatus &&
        supervised !== undefined &&
        supervised === (await servingPid())
      ) {
        return report;
      }
    } catch {
    }
    await Bun.sleep(exitIntervalMs);
  }
  return undefined;
}

async function handBackToLaunchd(report: ServiceReport): Promise<void> {
  const uid = process.getuid?.() ?? 0;
  const kicked = await capture([
    "/bin/launchctl",
    "kickstart",
    `gui/${uid}/${report.label}`,
  ]);
  if (kicked.code !== 0) {
    throw new Error(`cannot hand '${report.label}' to launchd:\n${kicked.output}`);
  }
}

async function verifySupervision(
  binary: string,
  source: string,
): Promise<string> {
  const supervised = await waitForSupervision(binary, source);
  if (supervised !== undefined) {
    return `${supervised.label} (${supervised.kind}) is ${supervised.status}`;
  }
  const stalled = await readServiceReport(binary, source);
  if (stalled.kind !== launchdKind) {
    throw new Error(`the pm3 service is ${stalled.status} after installing`);
  }
  await handBackToLaunchd(stalled);
  const kicked = await waitForSupervision(binary, source);
  if (kicked === undefined) {
    throw new Error(`the pm3 service is ${stalled.status} after a kickstart`);
  }
  return `${kicked.label} (${kicked.kind}) is ${kicked.status} after a kickstart`;
}

export async function install(): Promise<number> {
  const { destination, backups, source } = installation();
  const built = await buildOptimisedRelease();
  if (built !== 0) {
    return built;
  }

  const before = await listedServices(destination, source);
  const stamp = join(backups, backupStamp(new Date()));
  await backUp([destination], stamp);
  const planned = await overwrittenByInstall(destination, source);
  await replaceBinary(destination);
  await backUp(planned, stamp);
  await announce("backed up", stamp);

  await announce("reinstalled", await reinstallService(destination, source));
  await announce("service", await verifySupervision(destination, source));

  const comparison = compareServices(
    before,
    await listedServices(destination, source),
  );
  await announce("services", describeComparison(comparison));
  if (comparison.lost.length > 0) {
    return 1;
  }
  return 0;
}

if (import.meta.main) {
  process.exit(await install());
}
