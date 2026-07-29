type MonitorKind = "business" | "crash";

const patternByKind: Record<MonitorKind, RegExp> = {
  business:
    /"result":"(error|timeout|http_error|api_error|decode_error|missing|server_error|client_error)"|"level":"(WARN|ERROR)"|panicked at/u,
  crash:
    /panicked at|stack backtrace|fatal runtime error|stack overflow|SIGABRT|SIGSEGV|thread .* panicked/u,
};

const defaultServiceLog = "scratchpad/skel_rs.log";

function exitWithUsage(): never {
  process.stderr.write("用法: just monitor {crash|business}\n");
  process.exit(2);
}

function parseKind(raw: string | undefined): MonitorKind {
  if (raw === "crash" || raw === "business") {
    return raw;
  }
  return exitWithUsage();
}

async function writeMatchingLines(
  source: ReadableStream<Uint8Array>,
  pattern: RegExp,
): Promise<void> {
  const decoder = new TextDecoder();
  let pending = "";
  for await (const chunk of source) {
    pending += decoder.decode(chunk, { stream: true });
    const complete = pending.split("\n");
    pending = complete.pop() ?? "";
    for (const line of complete) {
      if (pattern.test(line)) {
        process.stdout.write(`${line}\n`);
      }
    }
  }
}

export async function tailFilteredServiceLog(argv: string[]): Promise<number> {
  const kind = parseKind(argv[0]);
  const serviceLog = Bun.env["SERVICE_LOG"] ?? defaultServiceLog;
  const tail = Bun.spawn(["tail", "-F", "-n0", serviceLog], {
    stderr: "inherit",
    stdout: "pipe",
  });
  await writeMatchingLines(tail.stdout, patternByKind[kind]);
  return tail.exited;
}

if (import.meta.main) {
  process.exit(await tailFilteredServiceLog(process.argv.slice(2)));
}
