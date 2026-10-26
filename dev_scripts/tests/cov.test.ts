import { describe, expect, test } from "bun:test";

import {
  type CoverageReport,
  coverageGaps,
  describeGaps,
  executablesOf,
  ignoredFiles,
  instrumentationProblem,
  staleObjects,
} from "../cov.ts";

type Row = [string, number, number, number, number, number, number];

function report(files: Row[]): CoverageReport {
  return {
    data: [
      {
        files: files.map(
          ([filename, lines, coveredLines, branches, coveredBranches, regions, coveredRegions]) => ({
            filename,
            summary: {
              lines: { count: lines, covered: coveredLines },
              branches: { count: branches, covered: coveredBranches },
              regions: { count: regions, covered: coveredRegions },
            },
          }),
        ),
      },
    ],
  };
}

describe("coverageGaps", () => {
  test("keeps only files with an uncovered line, branch or region", () => {
    const gaps = coverageGaps(
      report([
        ["/repo/adapters/src/lib.rs", 10, 10, 4, 4, 6, 6],
        ["/repo/adapters/src/paths.rs", 10, 8, 4, 4, 6, 6],
        ["/repo/frameworks/src/cli.rs", 10, 10, 4, 3, 6, 6],
        ["/repo/frameworks/src/install.rs", 10, 10, 4, 4, 6, 5],
      ]),
      "/repo",
    );
    expect(gaps).toEqual([
      { file: "adapters/src/paths.rs", lines: 2, branches: 0, regions: 0 },
      { file: "frameworks/src/cli.rs", lines: 0, branches: 1, regions: 0 },
      { file: "frameworks/src/install.rs", lines: 0, branches: 0, regions: 1 },
    ]);
  });

  test("leaves a path outside the root as it is", () => {
    const gaps = coverageGaps(report([["/elsewhere/x.rs", 1, 0, 0, 0, 1, 1]]), "/repo");
    expect(gaps).toEqual([{ file: "/elsewhere/x.rs", lines: 1, branches: 0, regions: 0 }]);
  });
});

describe("describeGaps", () => {
  test("reports full coverage when nothing is missing", () => {
    expect(describeGaps([])).toBe("cov: 100% line, 100% branch, 100% region");
  });

  test("reports only line and branch gaps while any remain", () => {
    expect(
      describeGaps([
        { file: "a.rs", lines: 2, branches: 1, regions: 4 },
        { file: "b.rs", lines: 0, branches: 0, regions: 3 },
        { file: "c.rs", lines: 0, branches: 2, regions: 0 },
      ]),
    ).toBe("a.rs: 2 line(s), 1 branch(es) uncovered\nc.rs: 0 line(s), 2 branch(es) uncovered");
  });

  test("reports region gaps once lines and branches are full", () => {
    expect(
      describeGaps([
        { file: "b.rs", lines: 0, branches: 0, regions: 3 },
        { file: "d.rs", lines: 0, branches: 0, regions: 1 },
      ]),
    ).toBe("b.rs: 3 region(s) uncovered\nd.rs: 1 region(s) uncovered");
  });
});

describe("instrumentationProblem", () => {
  const crates = ["entities", "usecases", "adapters", "frameworks"];

  test("accepts a report covering every crate with branches", () => {
    const rows: Row[] = crates.map((crate) => [`/repo/${crate}/src/lib.rs`, 1, 1, 2, 2, 1, 1]);
    expect(instrumentationProblem(report(rows), "/repo")).toBeNull();
  });

  test("names the crates missing from the report", () => {
    const rows: Row[] = [["/repo/entities/src/lib.rs", 1, 1, 2, 2, 1, 1]];
    expect(instrumentationProblem(report(rows), "/repo")).toContain("usecases, adapters, frameworks");
  });

  test("rejects a report without any branch counter", () => {
    const rows: Row[] = crates.map((crate) => [`/repo/${crate}/src/lib.rs`, 1, 1, 0, 0, 1, 1]);
    expect(instrumentationProblem(report(rows), "/repo")).toContain("branch");
  });

  test("reads windows separators", () => {
    const rows: Row[] = crates.map((crate) => [`C:\\repo\\${crate}\\src\\lib.rs`, 1, 1, 2, 2, 1, 1]);
    expect(instrumentationProblem(report(rows), String.raw`C:\repo`)).toBeNull();
  });
});

describe("executablesOf", () => {
  test("collects executables from cargo json messages", () => {
    const lines = [
      JSON.stringify({ reason: "compiler-artifact", executable: "/t/debug/deps/a-0123456789abcdef" }),
      JSON.stringify({ reason: "compiler-artifact", executable: null }),
      JSON.stringify({ reason: "build-finished", success: true }),
      "not json",
      "",
    ].join("\n");
    expect(executablesOf(lines)).toEqual(["/t/debug/deps/a-0123456789abcdef"]);
  });
});

describe("staleObjects", () => {
  test("drops older builds of a current executable only", () => {
    const current = [
      { path: "/t/debug/build/a/0123456789abcdef/out/a-0123456789abcdef", id: "1" },
      { path: "/t/debug/pm3", id: "2" },
    ];
    const present = [
      { path: "/t/debug/build/a/0123456789abcdef/out/a-0123456789abcdef", id: "1" },
      { path: "/t/debug/build/a/fedcba9876543210/out/a-fedcba9876543210", id: "3" },
      { path: "/t/debug/build/a/fedcba9876543210/out/a-fedcba9876543210.d", id: "4" },
      { path: "/t/debug/deps/pm3-1111111111111111", id: "2" },
      { path: "/t/debug/deps/pm3-3333333333333333.exe", id: "5" },
      { path: "/t/debug/deps/serde-2222222222222222", id: "6" },
    ];
    expect(staleObjects(present, current)).toEqual([
      "/t/debug/build/a/fedcba9876543210/out/a-fedcba9876543210",
      "/t/debug/deps/pm3-3333333333333333.exe",
    ]);
  });
});

describe("ignoredFiles", () => {
  const ignored = new RegExp(ignoredFiles);

  test.each([
    "/repo/frameworks/tests/install.rs",
    "/repo/adapters/src/tests/paths_tests.rs",
    "/repo/frameworks/src/test_support/daemon_fixture_tests.rs",
    "/repo/arch_tests/tests/architecture.rs",
    "/repo/test_trace/src/lib.rs",
    String.raw`C:\repo\frameworks\tests\install.rs`,
  ])("skips test code at %s", (path) => {
    expect(ignored.test(path)).toBe(true);
  });

  test.each(["/repo/adapters/src/paths.rs", "/repo/frameworks/src/install.rs"])(
    "keeps production code at %s",
    (path) => {
      expect(ignored.test(path)).toBe(false);
    },
  );
});
