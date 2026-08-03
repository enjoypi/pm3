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

function defaultOf(source: string, key: string): string {
  const found = source.match(new RegExp(`${key}: "\\$\\{[A-Z0-9_]+:-([^}]+)\\}"`));
  if (found === null) {
    throw new Error(`${key} is no longer declared the way this guard reads it`);
  }
  return found[1] as string;
}

function armOf(source: string, variant: string): string {
  const found = source.match(
    new RegExp(`Self::${variant} => "([^"]+)"`),
  );
  if (found === null) {
    throw new Error(`${variant} is no longer rendered the way this guard reads it`);
  }
  return found[1] as string;
}

describe("install.ts mirrors the rust side", () => {
  test("the pid file name matches paths.rs", async () => {
    const paths = await sourceOf("adapters/src/paths.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const pidFileName = "${constantOf(paths, "PID_FILE")}";`,
    );
  });

  test("the default runtime home matches paths.rs", async () => {
    const paths = await sourceOf("adapters/src/paths.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    const home = constantOf(paths, "DEFAULT_HOME").replace("~/", "");
    expect(install).toContain(`const defaultRuntimeHome = "${home}";`);
  });

  test("the launchd kind matches unit/spec.rs", async () => {
    const spec = await sourceOf("adapters/src/unit/spec.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const launchdKind = "${armOf(spec, "Launchd")}";`,
    );
  });

  test("the running status matches unit/spec.rs", async () => {
    const spec = await sourceOf("adapters/src/unit/spec.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const runningStatus = "${armOf(spec, "Running")}";`,
    );
  });

  test("the systemd kind matches unit/spec.rs", async () => {
    const spec = await sourceOf("adapters/src/unit/spec.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const systemdKind = "${armOf(spec, "Systemd")}";`,
    );
  });

  test("the systemctl program matches the config default", async () => {
    const config = await sourceOf("config.yaml");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const systemctlProgram = "${defaultOf(config, "systemctl_path")}";`,
    );
  });
});

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
