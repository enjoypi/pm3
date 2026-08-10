import { describe, expect, test } from "bun:test";

const repoRoot = new URL("../../", import.meta.url).pathname;

async function sourceOf(relative: string): Promise<string> {
  return await Bun.file(`${repoRoot}${relative}`).text();
}

function constantOf(source: string, name: string): string {
  const found = source.match(
    new RegExp(`pub const ${name}: &str = "([^"]+)";`),
  );
  if (found === null) {
    throw new Error(`${name} is no longer declared the way this guard reads it`);
  }
  return found[1] as string;
}

describe("monitor.ts mirrors the rust side", () => {
  test("the daemon log file name matches paths.rs", async () => {
    const paths = await sourceOf("adapters/src/paths.rs");
    const monitor = await sourceOf("dev_scripts/monitor.ts");
    expect(monitor).toContain(
      `const daemonLogFile = "${constantOf(paths, "DAEMON_LOG_FILE")}";`,
    );
  });

  test("the default runtime home matches paths.rs", async () => {
    const paths = await sourceOf("adapters/src/paths.rs");
    const monitor = await sourceOf("dev_scripts/monitor.ts");
    const home = constantOf(paths, "DEFAULT_HOME").replace("~/", "");
    expect(monitor).toContain(`const defaultRuntimeHome = "${home}";`);
  });
});

describe("reap.ts mirrors the rust side", () => {
  test("the signalable pid floor matches kill_signaler.rs", async () => {
    const signaler = await sourceOf("adapters/src/process/kill_signaler.rs");
    const reap = await sourceOf("dev_scripts/reap.ts");
    const found = signaler.match(/const LOWEST_SIGNALABLE_PID: u32 = (\d+);/);
    if (found === null) {
      throw new Error(
        "LOWEST_SIGNALABLE_PID is no longer declared the way this guard reads it",
      );
    }
    expect(reap).toContain(`pid >= ${found[1]}`);
  });

  test("the signalable pid ceiling matches kill_signaler.rs", async () => {
    const signaler = await sourceOf("adapters/src/process/kill_signaler.rs");
    const reap = await sourceOf("dev_scripts/reap.ts");
    if (!signaler.includes("i32::try_from(pid).is_ok()")) {
      throw new Error(
        "is_signalable no longer bounds pids by i32 the way this guard reads it",
      );
    }
    expect(reap).toContain("const maxSignalablePid = 2_147_483_647;");
  });
});
