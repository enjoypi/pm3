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
