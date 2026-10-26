import { mkdir, readdir, readFile, rm, stat } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

import {
  captureCargo,
  coverageEnv,
  covTargetDir,
  runCargo,
} from "./cargo_invocation.ts";
import { reapOrphanedDaemons } from "./reap.ts";

const reportPath = "target/llvm-cov/coverage.json";
const instrumentedCrates = ["entities", "usecases", "adapters", "frameworks"];
const hashedObject = /^(.+)-[0-9a-f]{16}(\.exe)?$/;
const testArgs = ["--locked", "--all-targets", "--workspace"];
export const ignoredFiles =
  String.raw`([/\\]tests?[/\\]|_tests\.rs$|[/\\]arch_tests[/\\]|[/\\]test_trace[/\\])`;

interface Tally {
  count: number;
  covered: number;
}

interface FileCoverage {
  filename: string;
  summary: { lines: Tally; branches: Tally; regions: Tally };
}

export interface CoverageReport {
  data: { files: FileCoverage[] }[];
}

export interface CoverageGap {
  file: string;
  lines: number;
  branches: number;
  regions: number;
}

export function coverageGaps(report: CoverageReport, root: string): CoverageGap[] {
  return report.data
    .flatMap((export_) => export_.files)
    .map((file) => ({
      file: file.filename.startsWith(root)
        ? file.filename.slice(root.length + 1)
        : file.filename,
      lines: file.summary.lines.count - file.summary.lines.covered,
      branches: file.summary.branches.count - file.summary.branches.covered,
      regions: file.summary.regions.count - file.summary.regions.covered,
    }))
    .filter((gap) => gap.lines > 0 || gap.branches > 0 || gap.regions > 0);
}

function lineBranchText(gap: CoverageGap): string {
  return `${gap.file}: ${gap.lines} line(s), ${gap.branches} branch(es) uncovered`;
}

function regionText(gap: CoverageGap): string {
  return `${gap.file}: ${gap.regions} region(s) uncovered`;
}

export function describeGaps(gaps: readonly CoverageGap[]): string {
  const lineOrBranch = gaps.filter((gap) => gap.lines > 0 || gap.branches > 0);
  if (lineOrBranch.length > 0) {
    return lineOrBranch.map(lineBranchText).join("\n");
  }
  if (gaps.length > 0) {
    return gaps.map(regionText).join("\n");
  }
  return "cov: 100% line, 100% branch, 100% region";
}

export function instrumentationProblem(report: CoverageReport, root: string): string | null {
  const files = report.data.flatMap((export_) => export_.files);
  const relative = files.map((file) =>
    file.filename.slice(root.length + 1).replaceAll("\\", "/"),
  );
  const missing = instrumentedCrates.filter(
    (crate) => !relative.some((path) => path.startsWith(`${crate}/src/`)),
  );
  if (missing.length > 0) {
    return `cov: 报告缺少 ${missing.join(", ")}，疑似复用了未插桩产物`;
  }
  if (files.every((file) => file.summary.branches.count === 0)) {
    return "cov: 报告没有任何 branch 计数，疑似复用了未插桩产物";
  }
  return null;
}

export function executablesOf(messages: string): string[] {
  const found: string[] = [];
  for (const line of messages.split("\n")) {
    try {
      const message = JSON.parse(line) as { reason?: string; executable?: string | null };
      if (message.reason === "compiler-artifact" && typeof message.executable === "string") {
        found.push(message.executable);
      }
    } catch {
      continue;
    }
  }
  return found;
}

function stemOf(path: string): string {
  const name = basename(path);
  return name.match(hashedObject)?.[1] ?? name;
}

export interface ObjectFile {
  path: string;
  id: string;
}

export function staleObjects(
  present: readonly ObjectFile[],
  current: readonly ObjectFile[],
): string[] {
  const stems = new Set(current.map((file) => stemOf(file.path)));
  const kept = new Set(current.map((file) => file.id));
  return present
    .filter(
      (file) =>
        hashedObject.test(basename(file.path)) &&
        stems.has(stemOf(file.path)) &&
        !kept.has(file.id),
    )
    .map((file) => file.path);
}

async function reap(): Promise<void> {
  await reapOrphanedDaemons(join(process.cwd(), "target"));
}

async function identify(paths: readonly string[]): Promise<ObjectFile[]> {
  const files: ObjectFile[] = [];
  for (const path of paths) {
    const found = await stat(path).catch(() => null);
    if (found?.isFile() === true) {
      files.push({ path, id: `${found.dev}:${found.ino}` });
    }
  }
  return files;
}

async function pruneStale(env: Record<string, string>): Promise<number> {
  const built = await captureCargo(
    ["test", ...testArgs, "--no-run", "--message-format=json"],
    env,
  );
  if (built.code !== 0) {
    return built.code;
  }
  const debug = join(covTargetDir(process.cwd()), "debug");
  const names = await readdir(debug, { recursive: true });
  const hashed = names.filter((name) => hashedObject.test(basename(name)));
  const present = await identify(hashed.map((name) => join(debug, name)));
  const current = await identify(executablesOf(built.stdout));
  for (const path of staleObjects(present, current)) {
    await rm(path, { force: true });
  }
  return 0;
}

async function runTests(env: Record<string, string>): Promise<number> {
  const cleaned = await runCargo(["llvm-cov", "clean", "--profraw-only"], env);
  if (cleaned !== 0) {
    return cleaned;
  }
  await reap();
  const tested = await runCargo(
    ["nextest", "run", ...testArgs, ...Bun.argv.slice(2)],
    env,
  );
  await reap();
  return tested !== 0 ? tested : await pruneStale(env);
}

async function readReport(env: Record<string, string>): Promise<CoverageReport | number> {
  await mkdir(dirname(reportPath), { recursive: true });
  const reported = await runCargo(
    [
      "llvm-cov",
      "report",
      "--branch",
      "--ignore-filename-regex",
      ignoredFiles,
      "--json",
      "--summary-only",
      "--output-path",
      reportPath,
    ],
    env,
  );
  if (reported !== 0) {
    return reported;
  }
  return JSON.parse(await readFile(reportPath, "utf8")) as CoverageReport;
}

async function main(): Promise<number> {
  const env = await coverageEnv(process.cwd());
  const tested = await runTests(env);
  if (tested !== 0) {
    return tested;
  }
  const report = await readReport(env);
  if (typeof report === "number") {
    return report;
  }
  const problem = instrumentationProblem(report, process.cwd());
  if (problem !== null) {
    process.stderr.write(`${problem}\n`);
    return 1;
  }
  const gaps = coverageGaps(report, process.cwd());
  process.stdout.write(`${describeGaps(gaps)}\n`);
  return gaps.length === 0 ? 0 : 1;
}

if (import.meta.main) {
  process.exit(await main());
}
