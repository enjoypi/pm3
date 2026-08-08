import { describe, expect, test } from "bun:test";

import {
  descendantsOf,
  hasFixtureMarker,
  parseDaemonArgs,
  parseProcessTable,
  planReap,
  type ProcessRow,
} from "../reap.ts";

const targetRoot = "/repo/target";
const fixtureExe = `${targetRoot}/llvm-cov-target/debug/pm3`;

function rowOf(pid: number, ppid: number, args: string): ProcessRow {
  return { pid, ppid, args };
}

function daemonRow(
  pid: number,
  ppid: number,
  exe: string,
  config: string,
): ProcessRow {
  return rowOf(pid, ppid, `${exe} daemon --config ${config}`);
}

describe("parseProcessTable", () => {
  test("parses pid, ppid and args from ps output", () => {
    const text = [
      "  2068114       1 /repo/target/release/pm3 daemon --config /tmp/.tmpX/config.yaml",
      "2537958     855 /home/dev/bin/pm3 daemon --config /home/dev/.pm3/config.yaml",
      "",
      "not a process line",
      "123 abc malformed",
    ].join("\n");
    expect(parseProcessTable(text)).toEqual([
      {
        pid: 2068114,
        ppid: 1,
        args: "/repo/target/release/pm3 daemon --config /tmp/.tmpX/config.yaml",
      },
      {
        pid: 2537958,
        ppid: 855,
        args: "/home/dev/bin/pm3 daemon --config /home/dev/.pm3/config.yaml",
      },
    ]);
  });
});

describe("parseDaemonArgs", () => {
  test("extracts the exe and config path from a daemon command line", () => {
    expect(
      parseDaemonArgs(`${fixtureExe} daemon --config /tmp/.tmpX/config.yaml`),
    ).toEqual({ exe: fixtureExe, configPath: "/tmp/.tmpX/config.yaml" });
  });

  test("keeps config paths that contain spaces", () => {
    expect(
      parseDaemonArgs(`${fixtureExe} daemon --config /tmp/my dir/config.yaml`)
        ?.configPath,
    ).toBe("/tmp/my dir/config.yaml");
  });

  test("rejects non-daemon invocations", () => {
    expect(parseDaemonArgs(`${fixtureExe} list`)).toBeNull();
    expect(parseDaemonArgs(`${fixtureExe} daemon`)).toBeNull();
    expect(parseDaemonArgs(`${fixtureExe} daemon --config`)).toBeNull();
  });

  test("rejects executables that are not pm3", () => {
    expect(
      parseDaemonArgs("/usr/bin/vim daemon --config /tmp/.tmpX/config.yaml"),
    ).toBeNull();
  });
});

describe("hasFixtureMarker", () => {
  test("matches the e2e service label and fixture names", () => {
    expect(hasFixtureMarker('label: "pm3-e2e-never-installed"')).toBe(true);
    expect(hasFixtureMarker("name: pm3-fixture-api")).toBe(true);
  });

  test("rejects an ordinary config", () => {
    expect(hasFixtureMarker('label: "com.pm3.daemon"')).toBe(false);
  });
});

describe("descendantsOf", () => {
  test("walks children and grandchildren", () => {
    const rows = [
      rowOf(100, 1, "daemon"),
      rowOf(200, 100, "sh -c exec sleep 30"),
      rowOf(300, 200, "sleep 30"),
      rowOf(400, 1, "unrelated"),
    ];
    expect(descendantsOf(rows, 100)).toEqual([200, 300]);
  });

  test("skips pids that are not signalable", () => {
    const rows = [rowOf(100, 1, "daemon"), rowOf(1, 100, "init lookalike")];
    expect(descendantsOf(rows, 100)).toEqual([]);
  });
});

describe("planReap", () => {
  const everythingIsFixture = () => true;

  test("reaps an orphaned fixture daemon with its services and dir", () => {
    const rows = [
      daemonRow(2068114, 1, fixtureExe, "/tmp/.tmpX/config.yaml"),
      rowOf(3102201, 2068114, "sleep 30"),
    ];
    const plan = planReap(rows, targetRoot, everythingIsFixture);
    expect(plan.daemons).toEqual([
      { pid: 2068114, configPath: "/tmp/.tmpX/config.yaml" },
    ]);
    expect(plan.servicePids).toEqual([3102201]);
    expect(plan.dirs).toEqual(["/tmp/.tmpX"]);
  });

  test("keeps daemons that still have a live test parent", () => {
    const rows = [daemonRow(2068114, 7777, fixtureExe, "/tmp/.tmpX/config.yaml")];
    expect(planReap(rows, targetRoot, everythingIsFixture).daemons).toEqual([]);
  });

  test("keeps daemons outside the workspace target dir", () => {
    const rows = [
      daemonRow(
        2537958,
        1,
        "/home/dev/bin/pm3",
        "/home/dev/.pm3/config.yaml",
      ),
    ];
    expect(planReap(rows, targetRoot, everythingIsFixture).daemons).toEqual([]);
  });

  test("keeps daemon homes that are not e2e fixtures", () => {
    const rows = [daemonRow(2068114, 1, fixtureExe, "/tmp/manual/config.yaml")];
    expect(planReap(rows, targetRoot, () => false).daemons).toEqual([]);
  });
});
