const cargoFlagsVariable = "CARGO_FLAGS";

export function parseCargoFlags(raw: string | undefined): string[] {
  if (raw === undefined) {
    return [];
  }
  return raw.split(" ").filter((flag) => flag.length > 0);
}

export function cargoFlagsFromEnvironment(): string[] {
  return parseCargoFlags(Bun.env[cargoFlagsVariable]);
}

export async function runCargo(
  args: readonly string[],
  env: Readonly<Record<string, string>> = {},
): Promise<number> {
  const cargo = Bun.spawn(["cargo", ...args], {
    env: { ...Bun.env, ...env },
    stderr: "inherit",
    stdin: "inherit",
    stdout: "inherit",
  });
  return cargo.exited;
}
