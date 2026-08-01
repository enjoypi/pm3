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

  test("the launchd kind matches service/spec.rs", async () => {
    const spec = await sourceOf("adapters/src/service/spec.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const launchdKind = "${armOf(spec, "Launchd")}";`,
    );
  });

  test("the running status matches service/spec.rs", async () => {
    const spec = await sourceOf("adapters/src/service/spec.rs");
    const install = await sourceOf("dev_scripts/install.ts");
    expect(install).toContain(
      `const runningStatus = "${armOf(spec, "Running")}";`,
    );
  });
});
