import { describe, expect, test } from "bun:test";

import {
  backupStamp,
  compareServices,
  describeComparison,
  parseLaunchdPid,
  parseListedServices,
  parsePidFile,
  parseServiceReport,
  parseWriteTargets,
  runtimeDirectory,
} from "../install_plan.ts";

const installedReport = [
  "com.enjoypi.pm3 (launchd service): running",
  "/Users/dev/Library/LaunchAgents/com.enjoypi.pm3.plist",
].join("\n");

const listing = [
  "id  name           pid    status  ↺  uptime  sandbox",
  "0   mihomo-rule    48819  online  0  2h      workspace-write+net",
  "1   mihomo-global  6344   online  0  2h      workspace-write+net",
].join("\n");

describe("parseServiceReport", () => {
  test("reads the label, kind, status and unit path", () => {
    expect(parseServiceReport(installedReport)).toEqual({
      label: "com.enjoypi.pm3",
      kind: "launchd",
      status: "running",
      unitPath: "/Users/dev/Library/LaunchAgents/com.enjoypi.pm3.plist",
    });
  });

  test("reads a status that carries a comma", () => {
    const report = [
      "com.enjoypi.pm3 (systemd service): installed, not running",
      "/home/dev/.config/systemd/user/com.enjoypi.pm3.service",
    ].join("\n");
    expect(parseServiceReport(report)?.status).toBe("installed, not running");
  });

  test("yields nothing for output it cannot recognise", () => {
    expect(parseServiceReport("cannot resolve the config path")).toBeUndefined();
  });
});

describe("parseWriteTargets", () => {
  test("collects every path the plan writes", () => {
    const plan = [
      "write /Users/dev/.pm3/config.yaml",
      "pm3:",
      '  home: "~/.pm3"',
      "",
      "write /Users/dev/Library/LaunchAgents/com.enjoypi.pm3.plist",
      "<plist/>",
      "",
      "run /bin/launchctl load -w /Users/dev/Library/LaunchAgents/com.enjoypi.pm3.plist",
    ].join("\n");
    expect(parseWriteTargets(plan)).toEqual([
      "/Users/dev/.pm3/config.yaml",
      "/Users/dev/Library/LaunchAgents/com.enjoypi.pm3.plist",
    ]);
  });

  test("ignores a plan line that only mentions writing inside contents", () => {
    const plan = ["write /tmp/a.yaml", "  note: write something"].join("\n");
    expect(parseWriteTargets(plan)).toEqual(["/tmp/a.yaml"]);
  });

  test("yields nothing for an empty plan", () => {
    expect(parseWriteTargets("")).toEqual([]);
  });
});

describe("parseListedServices", () => {
  test("reads every row of the table", () => {
    expect(parseListedServices(listing)).toEqual([
      { id: 0, name: "mihomo-rule", pid: 48819, status: "online" },
      { id: 1, name: "mihomo-global", pid: 6344, status: "online" },
    ]);
  });

  test("reads a stopped row without a pid", () => {
    const stopped = [
      "id  name  pid  status   ↺  uptime  sandbox",
      "0   web   -    stopped  1  -       workspace-write",
    ].join("\n");
    expect(parseListedServices(stopped)).toEqual([
      { id: 0, name: "web", pid: undefined, status: "stopped" },
    ]);
  });

  test("yields nothing when no apps are managed", () => {
    expect(parseListedServices("no apps are managed")).toEqual([]);
  });

  test("yields nothing for a failed command", () => {
    expect(parseListedServices("cannot parse config: missing field")).toEqual(
      [],
    );
  });
});

describe("compareServices", () => {
  test("counts a service that kept its pid as adopted", () => {
    const before = parseListedServices(listing);
    const comparison = compareServices(before, before);
    expect(comparison.adopted).toEqual(["mihomo-rule", "mihomo-global"]);
    expect(comparison.restarted).toEqual([]);
    expect(comparison.lost).toEqual([]);
  });

  test("counts a service whose pid changed as restarted", () => {
    const before = parseListedServices(listing);
    const after = before.map((row) =>
      row.name === "mihomo-rule" ? { ...row, pid: 999 } : row,
    );
    const comparison = compareServices(before, after);
    expect(comparison.restarted).toEqual(["mihomo-rule"]);
    expect(comparison.adopted).toEqual(["mihomo-global"]);
  });

  test("counts a service missing afterwards as lost", () => {
    const before = parseListedServices(listing);
    const after = before.filter((row) => row.name !== "mihomo-rule");
    expect(compareServices(before, after).lost).toEqual(["mihomo-rule"]);
  });

  test("counts a service that was not running before as restarted once it runs", () => {
    const before = [
      { id: 0, name: "web", pid: undefined, status: "stopped" },
    ];
    const after = [{ id: 0, name: "web", pid: 42, status: "online" }];
    expect(compareServices(before, after).restarted).toEqual(["web"]);
  });

  test("ignores a service that appeared out of nowhere", () => {
    const after = parseListedServices(listing);
    const comparison = compareServices([], after);
    expect(comparison.adopted).toEqual([]);
    expect(comparison.lost).toEqual([]);
  });
});

describe("describeComparison", () => {
  test("names the services that kept their process", () => {
    const before = parseListedServices(listing);
    const described = describeComparison(compareServices(before, before));
    expect(described).toContain("adopted 2");
    expect(described).toContain("mihomo-rule");
  });

  test("says so when nothing was managed", () => {
    expect(describeComparison(compareServices([], []))).toContain(
      "no managed services",
    );
  });
});

describe("parseLaunchdPid", () => {
  test("reads the pid launchd reports for a supervised job", () => {
    const listing = ['{', '\t"PID" = 1063;', '\t"LastExitStatus" = 0;', "};"].join(
      "\n",
    );
    expect(parseLaunchdPid(listing)).toBe(1063);
  });

  test("yields nothing when launchd supervises no process", () => {
    const listing = ["{", '\t"LastExitStatus" = 0;', "};"].join("\n");
    expect(parseLaunchdPid(listing)).toBeUndefined();
  });
});

describe("parsePidFile", () => {
  test("reads a pid written by the daemon", () => {
    expect(parsePidFile("1072\n")).toBe(1072);
  });

  test("yields nothing for a half written file", () => {
    expect(parsePidFile("")).toBeUndefined();
  });
});

describe("backupStamp", () => {
  test("renders a sortable UTC stamp", () => {
    expect(backupStamp(new Date("2026-07-30T13:33:44.512Z"))).toBe(
      "20260730T133344Z",
    );
  });
});

describe("runtimeDirectory", () => {
  test("keeps the runtime directory a login session already declared", () => {
    expect(runtimeDirectory("/run/user/1000", 4242)).toBe("/run/user/1000");
  });

  test("derives the runtime directory a non-login shell never got", () => {
    expect(runtimeDirectory(undefined, 4242)).toBe("/run/user/4242");
  });

  test("treats an empty declaration as no declaration", () => {
    expect(runtimeDirectory("", 4242)).toBe("/run/user/4242");
  });
});
