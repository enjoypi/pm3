import { describe, expect, test } from "bun:test";

import {
  collectInstrumentedFiles,
  definesImplementationItems,
  findFilesBelowFullCoverage,
  findZeroHitEntries,
} from "../coverage_gate.ts";

function lcovRecord(path: string, counters: readonly string[]): string {
  return [`SF:${path}`, ...counters, "end_of_record"].join("\n");
}

describe("findFilesBelowFullCoverage", () => {
  test("reports nothing when every metric reached full coverage", () => {
    const lcov = lcovRecord("/repo/entities/src/lib.rs", [
      "LF:20",
      "LH:20",
      "FNF:2",
      "FNH:2",
      "BRF:4",
      "BRH:4",
    ]);
    expect(findFilesBelowFullCoverage(lcov)).toEqual([]);
  });

  test("reports nothing for a file with no branches at all", () => {
    const lcov = lcovRecord("/repo/usecases/src/lib.rs", [
      "LF:20",
      "LH:20",
      "FNF:2",
      "FNH:2",
      "BRF:0",
      "BRH:0",
    ]);
    expect(findFilesBelowFullCoverage(lcov)).toEqual([]);
  });

  test("names each metric that fell short", () => {
    const lcov = lcovRecord("/repo/adapters/src/config.rs", [
      "LF:20",
      "LH:18",
      "FNF:2",
      "FNH:2",
      "BRF:4",
      "BRH:1",
    ]);
    expect(findFilesBelowFullCoverage(lcov)).toEqual([
      "/repo/adapters/src/config.rs: lines 18/20, branches 1/4",
    ]);
  });

  test("keeps counters separate per source file", () => {
    const lcov = [
      lcovRecord("/repo/a.rs", ["LF:2", "LH:2"]),
      lcovRecord("/repo/b.rs", ["LF:2", "LH:1"]),
    ].join("\n");
    expect(findFilesBelowFullCoverage(lcov)).toEqual(["/repo/b.rs: lines 1/2"]);
  });

  test("reports nothing for empty lcov data", () => {
    expect(findFilesBelowFullCoverage("")).toEqual([]);
  });
});

describe("findZeroHitEntries", () => {
  test("reports an uncovered line together with its source file", () => {
    const lcov = ["SF:/repo/adapters/src/config.rs", "DA:12,0", "end_of_record"];
    expect(findZeroHitEntries(lcov.join("\n"))).toEqual([
      "SF:/repo/adapters/src/config.rs: DA:12,0",
    ]);
  });

  test("reports an uncovered branch", () => {
    const lcov = ["SF:/repo/adapters/src/config.rs", "BRDA:12,0,1,0"];
    expect(findZeroHitEntries(lcov.join("\n"))).toEqual([
      "SF:/repo/adapters/src/config.rs: BRDA:12,0,1,0",
    ]);
  });

  test("ignores entries whose hit count is not zero", () => {
    const lcov = ["SF:/repo/adapters/src/config.rs", "DA:12,3", "BRDA:12,0,1,7"];
    expect(findZeroHitEntries(lcov.join("\n"))).toEqual([]);
  });

  test("attributes each entry to the most recent source file", () => {
    const lcov = [
      "SF:/repo/a.rs",
      "DA:1,1",
      "SF:/repo/b.rs",
      "DA:2,0",
    ];
    expect(findZeroHitEntries(lcov.join("\n"))).toEqual(["SF:/repo/b.rs: DA:2,0"]);
  });
});

describe("definesImplementationItems", () => {
  test.each([
    ["fn free()", "fn free() {}"],
    ["pub fn", "pub fn exported() {}"],
    ["pub(crate) fn", "pub(crate) fn scoped() {}"],
    ["async fn main", "#[tokio::main]\nasync fn main() {}"],
    ["pub async fn", "pub async fn ctrl_c_signal() {}"],
    ["pub(crate) async fn", "pub(crate) async fn probe() {}"],
    ["pub const fn", "pub const fn validate() {}"],
    ["impl block", "impl Example {\n    fn helper() {}\n}"],
    ["generic impl block", "impl<T> Store<T> {}"],
  ])("counts %s as an implementation item", (_label, source) => {
    expect(definesImplementationItems(source)).toBe(true);
  });

  test.each([
    ["module declarations only", "pub mod cli;\npub mod telemetry;\n"],
    ["trait without bodies", "pub trait Store {\n    fn create(&self);\n}"],
    ["struct only", "pub struct Example {\n    pub id: i64,\n}"],
  ])("does not count %s", (_label, source) => {
    expect(definesImplementationItems(source)).toBe(false);
  });
});

describe("collectInstrumentedFiles", () => {
  test("strips the workspace prefix from instrumented paths", () => {
    const lcov = ["SF:/repo/entities/src/lib.rs", "SF:/repo/usecases/src/lib.rs"];
    expect(collectInstrumentedFiles(lcov.join("\n"), "/repo")).toEqual(
      new Set(["entities/src/lib.rs", "usecases/src/lib.rs"]),
    );
  });

  test("ignores source files outside the workspace root", () => {
    const lcov = ["SF:/elsewhere/vendor/lib.rs"];
    expect(collectInstrumentedFiles(lcov.join("\n"), "/repo")).toEqual(new Set());
  });
});
