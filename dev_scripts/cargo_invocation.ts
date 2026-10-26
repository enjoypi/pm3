import { join } from "node:path";

export const toolchain = "+nightly";
const covFlag = "--cov";
const exportLine = /^export ([A-Za-z_][A-Za-z0-9_]*)=(.*)$/;

export function covTargetDir(root: string): string {
  return join(root, "target", "llvm-cov-target");
}

function unquote(raw: string): string {
  let value = "";
  let quoted = false;
  for (let index = 0; index < raw.length; index += 1) {
    const char = raw[index] as string;
    if (char === "'") {
      quoted = !quoted;
    } else if (char === "\\" && !quoted) {
      index += 1;
      value += raw[index] ?? "";
    } else {
      value += char;
    }
  }
  return value;
}

export function parseShowEnv(text: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const found = line.match(exportLine);
    if (found !== null) {
      env[found[1] as string] = unquote(found[2] as string);
    }
  }
  return env;
}

export function splitCovFlag(argv: readonly string[]): { cov: boolean; args: string[] } {
  const cov = argv[0] === covFlag;
  return { cov, args: argv.slice(cov ? 1 : 0) };
}

export async function coverageEnv(root: string): Promise<Record<string, string>> {
  const target = { CARGO_TARGET_DIR: covTargetDir(root) };
  const shown = Bun.spawn(["cargo", toolchain, "llvm-cov", "show-env", "--branch", "--sh"], {
    env: { ...Bun.env, ...target },
    stderr: "ignore",
    stdout: "pipe",
  });
  const text = await new Response(shown.stdout).text();
  if ((await shown.exited) !== 0) {
    throw new Error("cargo llvm-cov show-env 失败");
  }
  return { ...parseShowEnv(text), ...target };
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

export async function captureCargo(
  args: readonly string[],
  env: Readonly<Record<string, string>>,
): Promise<{ code: number; stdout: string }> {
  const cargo = Bun.spawn(["cargo", ...args], {
    env: { ...Bun.env, ...env },
    stderr: "inherit",
    stdout: "pipe",
  });
  const stdout = await new Response(cargo.stdout).text();
  return { code: await cargo.exited, stdout };
}

if (import.meta.main) {
  const { cov, args } = splitCovFlag(Bun.argv.slice(2));
  const env = cov ? await coverageEnv(process.cwd()) : {};
  process.exit(await runCargo([toolchain, ...args], env));
}
