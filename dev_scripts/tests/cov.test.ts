import { describe, expect, test } from "bun:test";

import { type CoverageReport, coverageGaps, describeGaps, ignoredFiles } from "../cov.ts";

function report(files: [string, number, number, number, number][]): CoverageReport {
  return {
    data: [
      {
        files: files.map(([filename, lines, coveredLines, branches, coveredBranches]) => ({
          filename,
          summary: {
            lines: { count: lines, covered: coveredLines },
            branches: { count: branches, covered: coveredBranches },
          },
        })),
      },
    ],
  };
}

describe("coverageGaps", () => {
  test("keeps only files with an uncovered line or branch", () => {
    const gaps = coverageGaps(
      report([
        ["/repo/adapters/src/lib.rs", 10, 10, 4, 4],
        ["/repo/adapters/src/paths.rs", 10, 8, 4, 4],
        ["/repo/frameworks/src/cli.rs", 10, 10, 4, 3],
      ]),
      "/repo",
    );
    expect(gaps).toEqual([
      { file: "adapters/src/paths.rs", lines: 2, branches: 0 },
      { file: "frameworks/src/cli.rs", lines: 0, branches: 1 },
    ]);
  });

  test("leaves a path outside the root as it is", () => {
    const gaps = coverageGaps(report([["/elsewhere/x.rs", 1, 0, 0, 0]]), "/repo");
    expect(gaps).toEqual([{ file: "/elsewhere/x.rs", lines: 1, branches: 0 }]);
  });
});

describe("describeGaps", () => {
  test("reports full coverage when nothing is missing", () => {
    expect(describeGaps([])).toBe("cov: 100% line, 100% branch");
  });

  test("lists every file that misses coverage", () => {
    expect(
      describeGaps([
        { file: "a.rs", lines: 2, branches: 0 },
        { file: "b.rs", lines: 0, branches: 1 },
      ]),
    ).toBe(
      "a.rs: 2 line(s), 0 branch(es) uncovered\nb.rs: 0 line(s), 1 branch(es) uncovered",
    );
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
