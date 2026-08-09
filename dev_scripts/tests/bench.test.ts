import { describe, expect, test } from "bun:test";

import { configYaml, parseViews, parseVmRssKib, renderReport, summarize } from "../bench.ts";

describe("parseVmRssKib", () => {
  test("parses the VmRSS line out of /proc status", () => {
    const status = "Name:\tpm3\nState:\tS (sleeping)\nVmRSS:\t    4864 kB\nVmHWM:\t    5120 kB\n";
    expect(parseVmRssKib(status)).toBe(4864);
  });

  test("returns null when VmRSS is absent", () => {
    expect(parseVmRssKib("Name:\tpm3\n")).toBeNull();
  });
});

describe("summarize", () => {
  test("computes mean, median and p95 over sorted samples", () => {
    const summary = summarize([10, 1, 5, 7, 3]);
    expect(summary.count).toBe(5);
    expect(summary.mean).toBe(5);
    expect(summary.median).toBe(5);
    expect(summary.p95).toBe(10);
  });

  test("rejects empty input", () => {
    expect(() => summarize([])).toThrow(/at least one sample/);
  });
});

describe("parseViews", () => {
  test("parses list --json output into views", () => {
    const views = parseViews('[{"name":"web","status":"online","pid":42},{"name":"db","status":"stopped","pid":null}]');
    expect(views).toEqual([
      { name: "web", status: "online", pid: 42 },
      { name: "db", status: "stopped", pid: null },
    ]);
  });

  test("rejects non-array payloads", () => {
    expect(() => parseViews("{}")).toThrow(/did not answer an array/);
  });

  test("rejects entries without name/status", () => {
    expect(() => parseViews('[{"pid":1}]')).toThrow(/misses name\/status/);
  });
});

describe("configYaml", () => {
  test("embeds the home and the reap fixture marker", () => {
    const yaml = configYaml("/tmp/pm3-bench-x");
    expect(yaml).toContain('home: "/tmp/pm3-bench-x"');
    expect(yaml).toContain("pm3-fixture");
    expect(yaml).toContain('mode: "workspace-write"');
  });
});

describe("renderReport", () => {
  test("renders a markdown table with environment note", () => {
    const report = renderReport({
      pm3Version: "pm3 1.11.0",
      machine: "linux 7.0 x86_64, 8 核",
      dateUtc: "2026-08-09T00:00:00.000Z",
      coldStartMs: 120,
      reclaimStartMs: 200,
      idleRssKib: 4096,
      loadedRssKib: 8192,
      perServiceOverheadKib: 512,
      childrenRssKib: 10240,
      startMs: { count: 8, mean: 90, median: 88, p95: 110 },
      listMs: { count: 50, mean: 12, median: 11, p95: 20 },
    });
    expect(report).toContain("| 冷启动（含拉起 daemon） | 120 ms |");
    expect(report).toContain("mean 12 ms / median 11 ms / p95 20 ms");
    expect(report).toContain("4.0 MiB");
    expect(report).toContain("每服务开销 512 KiB");
    expect(report).toContain("just bench");
  });
});
