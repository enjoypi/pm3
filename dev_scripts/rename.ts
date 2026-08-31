import { readdir } from "node:fs/promises";
import { dirname, join, relative } from "node:path";

import { cargoFlagsFromEnvironment, runCargo } from "./cargo_invocation.ts";

interface RewrittenFile {
  readonly occurrences: number;
  readonly path: string;
}

const binManifest = join("frameworks", "Cargo.toml");
const skipDirectories = new Set([".git", "node_modules", "target"]);
const skipFiles = new Set(["Cargo.lock", "bun.lock"]);
const validCrateName = /^[a-z][a-z0-9_]*$/u;
const binSectionHeader = "[[bin]]";
const binNameEntry = /^name\s*=\s*"([^"]+)"$/u;
const utf8OrThrow = new TextDecoder("utf-8", { fatal: true });

const usage = [
  "用法: just rename <new_crate_name>",
  "",
  "旧名从 frameworks/Cargo.toml 的 [[bin]] name 动态解析，故本命令可重复执行。",
  "替换后自动跑 cargo check 验证。",
].join("\n");

function readBinName(manifest: string): string | undefined {
  let insideBinSection = false;
  for (const line of manifest.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[")) {
      insideBinSection = trimmed === binSectionHeader;
      continue;
    }
    if (!insideBinSection) {
      continue;
    }
    const matched = binNameEntry.exec(trimmed);
    if (matched?.[1] !== undefined) {
      return matched[1];
    }
  }
  return undefined;
}

async function* walkTextFiles(directory: string): AsyncGenerator<string> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (!skipDirectories.has(entry.name)) {
        yield* walkTextFiles(entryPath);
      }
    } else if (entry.isFile() && !skipFiles.has(entry.name)) {
      yield entryPath;
    }
  }
}

async function readUtf8(path: string): Promise<string | undefined> {
  try {
    return utf8OrThrow.decode(await Bun.file(path).bytes());
  } catch {
    return undefined;
  }
}

function countOccurrences(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1;
}

async function rewriteOccurrences(
  root: string,
  oldName: string,
  newName: string,
  selfPath: string,
): Promise<RewrittenFile[]> {
  const rewritten: RewrittenFile[] = [];
  for await (const path of walkTextFiles(root)) {
    if (path === selfPath) {
      continue;
    }
    const text = await readUtf8(path);
    if (text === undefined || !text.includes(oldName)) {
      continue;
    }
    await Bun.write(path, text.replaceAll(oldName, newName));
    rewritten.push({
      occurrences: countOccurrences(text, oldName),
      path: relative(root, path),
    });
  }
  return rewritten.sort((left, right) => (left.path < right.path ? -1 : 1));
}

function fail(reason: string): number {
  process.stderr.write(`cannot rename: ${reason}\n`);
  return 2;
}

async function renameProject(argv: readonly string[]): Promise<number> {
  const newName = argv[0];
  if (argv.length !== 1 || newName === undefined) {
    process.stderr.write(`${usage}\n`);
    return 2;
  }
  if (!validCrateName.test(newName)) {
    return fail(`${newName} 不是合法 crate 名（^[a-z][a-z0-9_]*$）`);
  }

  const root = dirname(import.meta.dir);
  const manifest = await readUtf8(join(root, binManifest));
  const oldName = manifest === undefined ? undefined : readBinName(manifest);
  if (oldName === undefined) {
    return fail(`无法从 ${binManifest} 的 [[bin]] 段解析当前项目名`);
  }
  if (oldName === newName) {
    return fail(`新名与当前项目名 ${oldName} 相同`);
  }

  const rewritten = await rewriteOccurrences(
    root,
    oldName,
    newName,
    import.meta.path,
  );
  if (rewritten.length === 0) {
    process.stdout.write(`未发现 ${oldName}，无需替换\n`);
    return 0;
  }
  for (const { occurrences, path } of rewritten) {
    process.stdout.write(`  ${path}: ${occurrences} 处\n`);
  }
  process.stdout.write(
    `共 ${rewritten.length} 个文件由 ${oldName} 替换为 ${newName}，运行 cargo check 验证…\n`,
  );

  const checked = await runCargo(["check", ...cargoFlagsFromEnvironment()]);
  if (checked !== 0) {
    process.stderr.write("cargo check 失败，请检查上方输出\n");
    return 1;
  }
  process.stdout.write(
    "完成。后续仍需人工改造：业务实体、migrations 断言、配置字段、feature 矩阵、arch_tests 依赖矩阵、CLI 子命令。\n",
  );
  return 0;
}

if (import.meta.main) {
  process.exit(await renameProject(process.argv.slice(2)));
}
