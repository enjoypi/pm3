import { describe, expect, test } from "bun:test";

import { parseShowEnv, splitCovFlag } from "../cargo_invocation.ts";

describe("parseShowEnv", () => {
  test("reads bare and single-quoted exports", () => {
    const text = [
      "export LLVM_PROFILE_FILE='/repo/target/llvm-cov-target/pm3-%p-%10m.profraw'",
      "export RUSTC_WRAPPER=/home/dev/.cargo/bin/cargo-llvm-cov",
      "",
    ].join("\n");
    expect(parseShowEnv(text)).toEqual({
      LLVM_PROFILE_FILE: "/repo/target/llvm-cov-target/pm3-%p-%10m.profraw",
      RUSTC_WRAPPER: "/home/dev/.cargo/bin/cargo-llvm-cov",
    });
  });

  test("unescapes an embedded single quote", () => {
    expect(parseShowEnv("export A='it'\\''s'")).toEqual({ A: "it's" });
  });

  test("keeps separators and equal signs inside the value", () => {
    expect(parseShowEnv("export F='-Ca=b\x1f--cfg=coverage'")).toEqual({
      F: "-Ca=b\x1f--cfg=coverage",
    });
  });

  test("ignores lines that are not exports", () => {
    expect(parseShowEnv("info: setting cfg(coverage)\nwarning: x")).toEqual({});
  });
});

describe("splitCovFlag", () => {
  test("detects a leading --cov", () => {
    expect(splitCovFlag(["--cov", "test", "-p", "x"])).toEqual({
      cov: true,
      args: ["test", "-p", "x"],
    });
  });

  test("passes other arguments through", () => {
    expect(splitCovFlag(["clippy", "--cov"])).toEqual({
      cov: false,
      args: ["clippy", "--cov"],
    });
  });
});
