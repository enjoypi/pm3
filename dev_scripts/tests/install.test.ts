import { afterEach, describe, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  listedServices,
  overwrittenByInstall,
  readServiceReport,
  systemdMainPid,
  waitForSupervision,
} from "../install.ts";
import { runtimeDirectory } from "../install_plan.ts";

const repoRoot = new URL("../../", import.meta.url).pathname;
const configSource = "config.yaml";

const sandboxes: string[] = [];
let previousPm3Home: string | undefined;

async function sandbox(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "pm3-install-test-"));
  sandboxes.push(dir);
  return dir;
}

async function substitute(
  dir: string,
  name: string,
  body: string,
): Promise<string> {
  const path = join(dir, name);
  await writeFile(path, `#!/bin/sh\n${body}\n`);
  await chmod(path, 0o755);
  return path;
}

async function recordedArgs(dir: string): Promise<string> {
  return await Bun.file(join(dir, "args")).text();
}

afterEach(async () => {
  if (previousPm3Home === undefined) {
    delete Bun.env["PM3_HOME"];
  } else {
    Bun.env["PM3_HOME"] = previousPm3Home;
  }
  previousPm3Home = undefined;
  while (sandboxes.length > 0) {
    const dir = sandboxes.pop();
    if (dir !== undefined) {
      await rm(dir, { force: true, recursive: true });
    }
  }
});

describe("listedServices", () => {
  test("passes the config source to pm3 list", async () => {
    const dir = await sandbox();
    const binary = await substitute(
      dir,
      "pm3",
      [
        `printf '%s\\n' "$@" > "${dir}/args"`,
        `printf '%s\\n' 'id  name  pid   status' '0   web   4242  online'`,
      ].join("\n"),
    );
    const rows = await listedServices(binary, configSource);
    expect(rows).toEqual([{ id: 0, name: "web", pid: 4242, status: "online" }]);
    expect(await recordedArgs(dir)).toBe(
      ["--config", configSource, "list", ""].join("\n"),
    );
  });

  test("rejects when pm3 list fails instead of yielding an empty table", async () => {
    const dir = await sandbox();
    const binary = await substitute(
      dir,
      "pm3",
      'echo "cannot parse config: missing field" >&2\nexit 1',
    );
    await expect(listedServices(binary, configSource)).rejects.toThrow(
      /cannot list the running pm3 services/,
    );
  });

  test("yields nothing when the binary is absent", async () => {
    const dir = await sandbox();
    expect(await listedServices(join(dir, "missing"), configSource)).toEqual(
      [],
    );
  });
});

describe("overwrittenByInstall", () => {
  test("yields nothing when the binary is absent", async () => {
    const dir = await sandbox();
    expect(
      await overwrittenByInstall(join(dir, "missing"), configSource),
    ).toEqual([]);
  });
});

describe("readServiceReport", () => {
  test("passes the config source to pm3 service", async () => {
    const dir = await sandbox();
    const binary = await substitute(
      dir,
      "pm3",
      [
        `printf '%s\\n' "$@" > "${dir}/args"`,
        `printf '%s\\n' 'dev.pm3 (systemd service): running' '/home/dev/.config/systemd/user/dev.pm3.service'`,
      ].join("\n"),
    );
    const report = await readServiceReport(binary, configSource);
    expect(report).toEqual({
      label: "dev.pm3",
      kind: "systemd",
      status: "running",
      unitPath: "/home/dev/.config/systemd/user/dev.pm3.service",
    });
    expect(await recordedArgs(dir)).toBe(
      ["--config", configSource, "service", ""].join("\n"),
    );
  });
});

describe("systemdMainPid", () => {
  test("reads the pid systemd reports for the unit", async () => {
    const dir = await sandbox();
    const systemctl = await substitute(
      dir,
      "systemctl",
      `printf '%s\\n' "$@" > "${dir}/args"\necho 4242`,
    );
    expect(await systemdMainPid(systemctl, "dev.pm3.service")).toBe(4242);
    expect(await recordedArgs(dir)).toBe(
      ["--user", "show", "-p", "MainPID", "--value", "dev.pm3.service", ""].join(
        "\n",
      ),
    );
  });

  test("hands systemd the runtime directory a non-login shell lacks", async () => {
    const dir = await sandbox();
    const expected = runtimeDirectory(
      Bun.env["XDG_RUNTIME_DIR"],
      process.getuid?.() ?? 0,
    );
    const systemctl = await substitute(
      dir,
      "systemctl",
      `test "$XDG_RUNTIME_DIR" = "${expected}" && echo 4242 || echo 0`,
    );
    expect(await systemdMainPid(systemctl, "dev.pm3.service")).toBe(4242);
  });

  test("treats a zero MainPID as unsupervised", async () => {
    const dir = await sandbox();
    const systemctl = await substitute(dir, "systemctl", "echo 0");
    expect(await systemdMainPid(systemctl, "dev.pm3.service")).toBeUndefined();
  });

  test("treats a failed query as unsupervised", async () => {
    const dir = await sandbox();
    const systemctl = await substitute(
      dir,
      "systemctl",
      'echo "Failed to connect to bus" >&2\nexit 1',
    );
    expect(await systemdMainPid(systemctl, "dev.pm3.service")).toBeUndefined();
  });

  test("treats output that is not a pid as unsupervised", async () => {
    const dir = await sandbox();
    const systemctl = await substitute(dir, "systemctl", "echo not-a-pid");
    expect(await systemdMainPid(systemctl, "dev.pm3.service")).toBeUndefined();
  });
});

describe("waitForSupervision", () => {
  test("retries through transient read failures until supervision holds", async () => {
    const dir = await sandbox();
    const binary = await substitute(
      dir,
      "pm3",
      [
        `count_file="${dir}/count"`,
        'n=0',
        'if [ -f "$count_file" ]; then n=$(cat "$count_file"); fi',
        "n=$((n + 1))",
        'printf \'%s\' "$n" > "$count_file"',
        'if [ "$n" -lt 3 ]; then',
        "  echo flaky >&2",
        "  exit 1",
        "fi",
        `printf '%s\\n' 'dev.pm3 (systemd service): running' '/home/dev/.config/systemd/user/dev.pm3.service'`,
      ].join("\n"),
    );
    const systemctl = await substitute(dir, "systemctl", "echo 4242");
    await writeFile(join(dir, "pm3.pid"), "4242\n");
    previousPm3Home = Bun.env["PM3_HOME"];
    Bun.env["PM3_HOME"] = dir;

    const report = await waitForSupervision(binary, configSource, systemctl);
    expect(report?.label).toBe("dev.pm3");
    const polls = await Bun.file(join(dir, "count")).text();
    expect(Number.parseInt(polls, 10)).toBeGreaterThanOrEqual(3);
  });
});

describe("install ordering", () => {
  test("plans the service backup with the old binary before replacing it", async () => {
    const source = await Bun.file(
      join(repoRoot, "dev_scripts/install.ts"),
    ).text();
    const planned = source.indexOf(
      "await overwrittenByInstall(destination, source)",
    );
    const replaced = source.indexOf("await replaceBinary(destination)");
    expect(planned).toBeGreaterThanOrEqual(0);
    expect(replaced).toBeGreaterThanOrEqual(0);
    expect(planned).toBeLessThan(replaced);
  });
});

describe("Dockerfile healthcheck", () => {
  test("probes the daemon process without the pm3 CLI", async () => {
    const dockerfile = await Bun.file(join(repoRoot, "Dockerfile")).text();
    expect(dockerfile).toContain("procps");
    const rest = dockerfile.slice(dockerfile.indexOf("HEALTHCHECK"));
    const healthcheck = rest.slice(0, rest.indexOf("\n\n") + 1 || undefined);
    expect(healthcheck).toContain('"pgrep"');
    expect(healthcheck).not.toContain('"/usr/local/bin/pm3"');
  });
});
