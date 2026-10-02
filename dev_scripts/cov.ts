import { mkdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";

import { runCargo } from "./cargo_invocation.ts";
import { reapOrphanedDaemons } from "./reap.ts";

const toolchain = "+nightly";
const reportPath = "target/llvm-cov/coverage.json";
export const ignoredFiles =
  String.raw`([/\\]tests?[/\\]|_tests\.rs$|[/\\]arch_tests[/\\]|[/\\]test_trace[/\\])`;

interface Tally {
  count: number;
  covered: number;
}

interface FileCoverage {
  filename: string;
  summary: { lines: Tally; branches: Tally };
}

export interface CoverageReport {
  data: { files: FileCoverage[] }[];
}

export interface CoverageGap {
  file: string;
  lines: number;
  branches: number;
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
    }))
    .filter((gap) => gap.lines > 0 || gap.branches > 0);
}

export function describeGaps(gaps: readonly CoverageGap[]): string {
  if (gaps.length === 0) {
    return "cov: 100% line, 100% branch";
  }
  return gaps
    .map((gap) => `${gap.file}: ${gap.lines} line(s), ${gap.branches} branch(es) uncovered`)
    .join("\n");
}

async function reap(): Promise<void> {
  await reapOrphanedDaemons(join(process.cwd(), "target"));
}

async function main(): Promise<number> {
  await runCargo([toolchain, "llvm-cov", "clean", "--workspace"]);
  await reap();
  const tested = await runCargo([
    toolchain,
    "llvm-cov",
    "--branch",
    "--locked",
    "--workspace",
    "--no-report",
    ...Bun.argv.slice(2),
  ]);
  await reap();
  if (tested !== 0) {
    return tested;
  }
  await mkdir(dirname(reportPath), { recursive: true });
  const reported = await runCargo([
    toolchain,
    "llvm-cov",
    "report",
    "--branch",
    "--ignore-filename-regex",
    ignoredFiles,
    "--json",
    "--summary-only",
    "--output-path",
    reportPath,
  ]);
  if (reported !== 0) {
    return reported;
  }
  const report = JSON.parse(await readFile(reportPath, "utf8")) as CoverageReport;
  const gaps = coverageGaps(report, process.cwd());
  process.stdout.write(`${describeGaps(gaps)}\n`);
  return gaps.length === 0 ? 0 : 1;
}

if (import.meta.main) {
  process.exit(await main());
}
