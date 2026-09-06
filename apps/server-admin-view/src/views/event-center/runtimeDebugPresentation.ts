import type { RuntimeDebugSample } from "@/types/runtime-debug";

export const summarizeDebugSamples = (samples: RuntimeDebugSample[]) => {
  let weightedCpu = 0;
  let measuredMs = 0;
  let maxCpu: number | null = null;
  const threads = new Map<
    number,
    { tid: number; name: string; cpuMs: number; peak: number }
  >();
  for (let index = 1; index < samples.length; ++index) {
    const sample = samples[index]!;
    const elapsed = Math.max(
      0,
      sample.elapsed_ms - samples[index - 1]!.elapsed_ms,
    );
    const cpu = sample.resource.cpu_percent;
    if (cpu == null || elapsed === 0) continue;
    weightedCpu += cpu * elapsed;
    measuredMs += elapsed;
    maxCpu = Math.max(maxCpu ?? 0, cpu);
    for (const thread of sample.resource.thread_cpu) {
      const previous = threads.get(thread.tid) ?? {
        tid: thread.tid,
        name: thread.name,
        cpuMs: 0,
        peak: 0,
      };
      previous.name = thread.name;
      previous.cpuMs += thread.cpu_percent * elapsed;
      previous.peak = Math.max(previous.peak, thread.cpu_percent);
      threads.set(thread.tid, previous);
    }
  }
  const rss = samples.flatMap((sample) =>
    sample.resource.rss_bytes == null ? [] : [sample.resource.rss_bytes],
  );
  return {
    averageCpu: measuredMs ? weightedCpu / measuredMs : null,
    maxCpu,
    rssDelta: rss.length > 1 ? rss[rss.length - 1]! - rss[0]! : null,
    threads: [...threads.values()]
      .map((thread) => ({
        ...thread,
        average: measuredMs ? thread.cpuMs / measuredMs : 0,
      }))
      .sort((left, right) => right.average - left.average)
      .slice(0, 8),
  };
};

export const formatDebugPercent = (value: number | null | undefined) =>
  value == null ? "—" : `${value.toFixed(2)}%`;
