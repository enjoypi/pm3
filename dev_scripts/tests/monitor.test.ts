import { describe, expect, test } from "bun:test";
import { join } from "node:path";

import { resolveServiceLog } from "../monitor.ts";

function envOf(
  entries: Record<string, string>,
): (name: string) => string | undefined {
  return (name) => entries[name];
}

describe("resolveServiceLog", () => {
  test("prefers the explicit SERVICE_LOG override", () => {
    const resolve = envOf({ PM3_HOME: "/srv/pm3", SERVICE_LOG: "/var/log/pm3.log" });
    expect(resolveServiceLog(resolve)).toBe("/var/log/pm3.log");
  });

  test("resolves the daemon log under PM3_HOME", () => {
    expect(resolveServiceLog(envOf({ PM3_HOME: "/srv/pm3" }))).toBe(
      join("/srv/pm3", "pm3.log"),
    );
  });

  test("falls back to the default runtime home under HOME", () => {
    expect(resolveServiceLog(envOf({ HOME: "/home/dev" }))).toBe(
      join("/home/dev", ".pm3", "pm3.log"),
    );
  });

  test("refuses to guess without any anchor", () => {
    expect(() => resolveServiceLog(() => undefined)).toThrow(
      /cannot locate the pm3 daemon log/,
    );
  });
});
