import { join } from "node:path";

type MonitorKind = "business" | "crash";

const daemonLogFile = "pm3.log";
const defaultStateHome = ".local/state";
const pm3Subdir = "pm3";
const serviceLogVariable = "SERVICE_LOG";
const homeVariable = "PM3_HOME";
const stateVariable = "PM3_STATE_DIR";
const xdgStateVariable = "XDG_STATE_HOME";
const userHomeVariable = "HOME";

const patternByKind: Record<MonitorKind, RegExp> = {
  business:
    /"result":"(error|timeout|http_error|api_error|decode_error|missing|server_error|client_error)"|"level":"(WARN|ERROR)"|panicked at/u,
  crash:
    /panicked at|stack backtrace|fatal runtime error|stack overflow|SIGABRT|SIGSEGV|thread .* panicked/u,
};

function named(
  env: (name: string) => string | undefined,
  variable: string,
): string | undefined {
  const value = env(variable);
  if (value === undefined || value.length === 0) {
    return undefined;
  }
  return value;
}

export function resolveServiceLog(
  env: (name: string) => string | undefined,
): string {
  const override = named(env, serviceLogVariable);
  if (override !== undefined) {
    return override;
  }
  const pm3Home = named(env, homeVariable);
  if (pm3Home !== undefined) {
    return join(pm3Home, daemonLogFile);
  }
  const stateDir = named(env, stateVariable);
  if (stateDir !== undefined) {
    return join(stateDir, daemonLogFile);
  }
  const xdgState = named(env, xdgStateVariable);
  if (xdgState !== undefined) {
    return join(xdgState, pm3Subdir, daemonLogFile);
  }
  const userHome = named(env, userHomeVariable);
  if (userHome !== undefined) {
    return join(userHome, defaultStateHome, pm3Subdir, daemonLogFile);
  }
  throw new Error(
    "cannot locate the pm3 daemon log: set SERVICE_LOG, PM3_HOME, PM3_STATE_DIR, XDG_STATE_HOME or HOME",
  );
}

function exitWithUsage(): never {
  process.stderr.write("用法: just monitor {crash|business}\n");
  process.exit(2);
}

function parseKind(raw: string | undefined): MonitorKind {
  if (raw === "crash" || raw === "business") {
    return raw;
  }
  return exitWithUsage();
}

async function writeMatchingLines(
  source: ReadableStream<Uint8Array>,
  pattern: RegExp,
): Promise<void> {
  const decoder = new TextDecoder();
  let pending = "";
  for await (const chunk of source) {
    pending += decoder.decode(chunk, { stream: true });
    const complete = pending.split("\n");
    pending = complete.pop() ?? "";
    for (const line of complete) {
      if (pattern.test(line)) {
        process.stdout.write(`${line}\n`);
      }
    }
  }
}

async function tailFilteredServiceLog(argv: string[]): Promise<number> {
  const kind = parseKind(argv[0]);
  const serviceLog = resolveServiceLog((name) => Bun.env[name]);
  const tail = Bun.spawn(["tail", "-F", "-n0", serviceLog], {
    stderr: "inherit",
    stdout: "pipe",
  });
  await writeMatchingLines(tail.stdout, patternByKind[kind]);
  return tail.exited;
}

if (import.meta.main) {
  process.exit(await tailFilteredServiceLog(process.argv.slice(2)));
}
