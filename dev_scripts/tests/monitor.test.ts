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

  test("resolves the daemon log under PM3_STATE_DIR", () => {
    expect(
      resolveServiceLog(envOf({ PM3_STATE_DIR: "/var/lib/pm3", HOME: "/home/dev" })),
    ).toBe(join("/var/lib/pm3", "pm3.log"));
  });

  test("prefers PM3_HOME over the split state root", () => {
    expect(
      resolveServiceLog(envOf({ PM3_HOME: "/srv/pm3", PM3_STATE_DIR: "/var/lib/pm3" })),
    ).toBe(join("/srv/pm3", "pm3.log"));
  });

  test("derives the daemon log from XDG_STATE_HOME", () => {
    expect(
      resolveServiceLog(envOf({ XDG_STATE_HOME: "/home/dev/state", HOME: "/home/dev" })),
    ).toBe(join("/home/dev/state", "pm3", "pm3.log"));
  });

  test("falls back to the xdg state default under HOME", () => {
    expect(resolveServiceLog(envOf({ HOME: "/home/dev" }))).toBe(
      join("/home/dev", ".local/state", "pm3", "pm3.log"),
    );
  });

  test("refuses to guess without any anchor", () => {
    expect(() => resolveServiceLog(() => undefined)).toThrow(
      /cannot locate the pm3 daemon log/,
    );
  });
});
