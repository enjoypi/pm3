import { describe, expect, test } from "bun:test";

import { parseCargoFlags } from "../cargo_invocation.ts";

describe("parseCargoFlags", () => {
  test("splits a space separated flag list", () => {
    expect(parseCargoFlags("--locked --release")).toEqual([
      "--locked",
      "--release",
    ]);
  });

  test("keeps a comma separated feature list as one flag", () => {
    expect(parseCargoFlags("--features=http,sqlite --workspace")).toEqual([
      "--features=http,sqlite",
      "--workspace",
    ]);
  });

  test("yields no flags when the variable is unset", () => {
    expect(parseCargoFlags(undefined)).toEqual([]);
  });

  test("yields no flags for an empty variable", () => {
    expect(parseCargoFlags("")).toEqual([]);
  });

  test("drops the padding produced by concatenating empty just variables", () => {
    expect(parseCargoFlags("  --locked   --release  ")).toEqual([
      "--locked",
      "--release",
    ]);
  });
});
