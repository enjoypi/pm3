import { rm } from "node:fs/promises";
import { join } from "node:path";

import { cargoFlagsFromEnvironment, runCargo } from "./cargo_invocation.ts";
import { reapOrphanedDaemons } from "./reap.ts";
import {
  findFilesBelowFullCoverage,
  findSourcesMissingFromLcov,
  findZeroHitEntries,
} from "./coverage_gate.ts";

const lcovOutput = "target/coverage.lcov";
const freshFlag = "--fresh";

const instrumentFlags = [
  "--branch",
  "--cargo-quiet",
  "--failure-output",
  "immediate-final",
  "--no-fail-fast",
  "--no-report",
  "--show-progress",
  "none",
  "--status-level",
  "fail",
  "--success-output",
  "never",
];

const failUnderFlags = [
  "--fail-under-file-lines",
  "99.999",
  "--fail-under-functions",
  "100",
  "--fail-under-lines",
  "100",
  "--fail-under-regions",
  "100",
];

function reportGateFailure(reason: string, details: readonly string[]): number {
  process.stderr.write(`coverage gate failed: ${reason}\n`);
  process.stdout.write(`${details.join("\n")}\n`);
  return 1;
}

async function instrumentAndExport(
  cargoFlags: readonly string[],
  fresh: boolean,
): Promise<number> {
  const cleaned = await runCargo([
    "+nightly",
    "llvm-cov",
    "clean",
    fresh ? "--workspace" : "--profraw-only",
  ]);
  if (cleaned !== 0) {
    return cleaned;
  }
  await rm(lcovOutput, { force: true });

  const instrumented = await runCargo([
    "+nightly",
    "llvm-cov",
    "nextest",
    ...cargoFlags,
    ...instrumentFlags,
  ]);
  if (instrumented !== 0) {
    return instrumented;
  }

  return runCargo([
    "+nightly",
    "llvm-cov",
    "report",
    "--release",
    "--lcov",
    "--output-path",
    lcovOutput,
    ...failUnderFlags,
  ]);
}

async function reapOrphansQuietly(): Promise<void> {
  try {
    await reapOrphanedDaemons(join(process.cwd(), "target"));
  } catch (error) {
    process.stderr.write(`e2e reap skipped: ${String(error)}\n`);
  }
}

async function enforceCoverageGate(
  argv: readonly string[],
): Promise<number> {
  await reapOrphansQuietly();
  try {
    return await runCoverageGate(argv);
  } finally {
    await reapOrphansQuietly();
  }
}

async function runCoverageGate(argv: readonly string[]): Promise<number> {
  const exported = await instrumentAndExport(
    cargoFlagsFromEnvironment(),
    argv.includes(freshFlag),
  );
  if (!(await Bun.file(lcovOutput).exists())) {
    return exported === 0 ? 1 : exported;
  }

  const lcov = await Bun.file(lcovOutput).text();
  const workspaceRoot = process.cwd();

  const below = findFilesBelowFullCoverage(lcov);
  if (below.length > 0) {
    process.stdout.write(`${below.join("\n")}\n`);
  }
  if (exported !== 0) {
    return exported;
  }

  const zeroHits = findZeroHitEntries(lcov);
  if (zeroHits.length > 0) {
    return reportGateFailure("lcov DA/BRDA ,0 entries below", zeroHits);
  }

  const missing = await findSourcesMissingFromLcov(lcov, workspaceRoot);
  if (missing.length > 0) {
    return reportGateFailure(
      "production source not in lcov (cov missing stats)",
      missing,
    );
  }

  process.stdout.write("COV_PASS\n");
  return 0;
}

if (import.meta.main) {
  process.exit(await enforceCoverageGate(process.argv.slice(2)));
}
