export interface ServiceReport {
  label: string;
  kind: string;
  status: string;
  unitPath: string;
}

export interface ServiceRow {
  id: number;
  name: string;
  pid: number | undefined;
  status: string;
}

export interface ServiceComparison {
  adopted: string[];
  restarted: string[];
  lost: string[];
}

const runtimeDirectoryRoot = "/run/user";
const backupDirectory = "install-backups";
const reportPattern = /^(.+) \((\w+) service\): (.+)$/;
const launchdPidPattern = /"PID"\s*=\s*(\d+)/;
const writePrefix = "write ";
const missingValue = "-";

export function parseLaunchdPid(listing: string): number | undefined {
  const matched = launchdPidPattern.exec(listing);
  const captured = matched?.[1];
  if (captured === undefined) {
    return undefined;
  }
  return Number.parseInt(captured, 10);
}

export function runtimeDirectory(
  declared: string | undefined,
  uid: number,
): string {
  if (declared !== undefined && declared.length > 0) {
    return declared;
  }
  return `${runtimeDirectoryRoot}/${uid}`;
}

export function backupRoot(
  declared: string | undefined,
  runtimeHome: string,
): string {
  if (declared !== undefined && declared.length > 0) {
    return declared;
  }
  return `${runtimeHome}/${backupDirectory}`;
}

export function parsePidFile(contents: string): number | undefined {
  const trimmed = contents.trim();
  if (!/^\d+$/.test(trimmed)) {
    return undefined;
  }
  return Number.parseInt(trimmed, 10);
}

export function parseMainPid(output: string): number | undefined {
  const pid = parsePidFile(output);
  if (pid === undefined || pid <= 0) {
    return undefined;
  }
  return pid;
}

export function parseServiceReport(report: string): ServiceReport | undefined {
  const [headline, unitPath] = report.trim().split("\n");
  if (headline === undefined || unitPath === undefined) {
    return undefined;
  }
  const matched = reportPattern.exec(headline.trim());
  if (matched === null) {
    return undefined;
  }
  const [, label, kind, status] = matched;
  if (label === undefined || kind === undefined || status === undefined) {
    return undefined;
  }
  return { label, kind, status, unitPath: unitPath.trim() };
}

export function parseWriteTargets(plan: string): string[] {
  return plan
    .split("\n")
    .filter((line) => line.startsWith(writePrefix))
    .map((line) => line.slice(writePrefix.length).trim())
    .filter((path) => path.length > 0);
}

export function parseListedServices(table: string): ServiceRow[] {
  return table.split("\n").flatMap(parseServiceRow);
}

export function compareServices(
  before: readonly ServiceRow[],
  after: readonly ServiceRow[],
): ServiceComparison {
  const comparison: ServiceComparison = {
    adopted: [],
    restarted: [],
    lost: [],
  };
  for (const row of before) {
    const survivor = after.find((candidate) => candidate.name === row.name);
    if (survivor === undefined) {
      comparison.lost.push(row.name);
    } else if (survivor.pid !== undefined && survivor.pid === row.pid) {
      comparison.adopted.push(row.name);
    } else {
      comparison.restarted.push(row.name);
    }
  }
  return comparison;
}

export function describeComparison(comparison: ServiceComparison): string {
  const { adopted, restarted, lost } = comparison;
  if (adopted.length + restarted.length + lost.length === 0) {
    return "no managed services to reclaim";
  }
  return [
    describeGroup("adopted", adopted),
    describeGroup("restarted", restarted),
    describeGroup("lost", lost),
  ]
    .filter((line) => line.length > 0)
    .join("\n");
}

export function backupStamp(now: Date): string {
  return now.toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
}

function describeGroup(label: string, names: readonly string[]): string {
  if (names.length === 0) {
    return "";
  }
  return `${label} ${names.length}: ${names.join(", ")}`;
}

function parseServiceRow(line: string): ServiceRow[] {
  const fields = line.trim().split(/\s+/);
  const [rawId, name, rawPid, status] = fields;
  if (rawId === undefined || name === undefined || rawPid === undefined) {
    return [];
  }
  if (status === undefined || !/^\d+$/.test(rawId)) {
    return [];
  }
  return [
    {
      id: Number.parseInt(rawId, 10),
      name,
      pid: rawPid === missingValue ? undefined : Number.parseInt(rawPid, 10),
      status,
    },
  ];
}
