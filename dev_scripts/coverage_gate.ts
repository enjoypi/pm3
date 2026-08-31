import { join } from "node:path";

const productionSourceRoots = [
  "entities/src",
  "usecases/src",
  "adapters/src",
  "frameworks/src",
];
const testFileSuffixes = ["_tests.rs", "_test_helpers.rs"];
const zeroHitEntry = /^(DA|BRDA):.*,0$/u;
const implementationItem =
  /^(impl[ <]|(?:pub(?:\([^)]*\))? )?(?:async |const |unsafe |extern )*fn )/mu;
const sourceFilePrefix = "SF:";
const recordTerminator = "end_of_record";
const lcovCounter = /^(LF|LH|FNF|FNH|BRF|BRH):(\d+)$/u;
const coverageMetrics = [
  ["lines", "LH", "LF"],
  ["functions", "FNH", "FNF"],
  ["branches", "BRH", "BRF"],
] as const;

function describeCoverageGap(counters: Map<string, number>): string {
  return coverageMetrics
    .filter(([, hit, found]) => (counters.get(hit) ?? 0) < (counters.get(found) ?? 0))
    .map(
      ([label, hit, found]) =>
        `${label} ${counters.get(hit) ?? 0}/${counters.get(found) ?? 0}`,
    )
    .join(", ");
}

export function findFilesBelowFullCoverage(lcov: string): string[] {
  const below: string[] = [];
  let currentSourceFile = "";
  let counters = new Map<string, number>();

  for (const line of lcov.split("\n")) {
    if (line.startsWith(sourceFilePrefix)) {
      currentSourceFile = line.slice(sourceFilePrefix.length);
      counters = new Map();
    } else if (line === recordTerminator) {
      const gap = describeCoverageGap(counters);
      if (gap.length > 0) {
        below.push(`${currentSourceFile}: ${gap}`);
      }
    } else {
      const counter = lcovCounter.exec(line);
      if (counter?.[1] !== undefined && counter[2] !== undefined) {
        counters.set(counter[1], Number(counter[2]));
      }
    }
  }

  return below;
}

export function findZeroHitEntries(lcov: string): string[] {
  const zeroHits: string[] = [];
  let currentSourceFile = "";
  for (const line of lcov.split("\n")) {
    if (line.startsWith(sourceFilePrefix)) {
      currentSourceFile = line;
    } else if (zeroHitEntry.test(line)) {
      zeroHits.push(`${currentSourceFile}: ${line}`);
    }
  }
  return zeroHits;
}

export function collectInstrumentedFiles(
  lcov: string,
  workspaceRoot: string,
): Set<string> {
  const stripped = `${sourceFilePrefix}${workspaceRoot}/`;
  const instrumented = new Set<string>();
  for (const line of lcov.split("\n")) {
    if (line.startsWith(stripped)) {
      instrumented.add(line.slice(stripped.length));
    }
  }
  return instrumented;
}

async function listProductionSources(): Promise<string[]> {
  const rustSources = new Bun.Glob("**/*.rs");
  const found: string[] = [];
  for (const root of productionSourceRoots) {
    for await (const discovered of rustSources.scan({ cwd: root })) {
      if (!testFileSuffixes.some((suffix) => discovered.endsWith(suffix))) {
        found.push(join(root, discovered));
      }
    }
  }
  return found;
}

export function definesImplementationItems(source: string): boolean {
  return implementationItem.test(source);
}

export async function findSourcesMissingFromLcov(
  lcov: string,
  workspaceRoot: string,
): Promise<string[]> {
  const instrumented = collectInstrumentedFiles(lcov, workspaceRoot);
  const candidates = (await listProductionSources()).filter(
    (source) => !instrumented.has(source),
  );
  const sources = await Promise.all(
    candidates.map((source) => Bun.file(source).text()),
  );
  const missing = candidates.filter((_, index) =>
    definesImplementationItems(sources[index] ?? ""),
  );
  return missing.sort((left, right) => (left < right ? -1 : 1));
}
