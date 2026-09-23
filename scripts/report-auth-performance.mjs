import { readFile } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";
import { compareRuns, comparisonKey } from "./auth-performance-lib.mjs";

const file = process.argv[2];
assert.ok(
  file,
  "usage: node scripts/report-auth-performance.mjs results.json > report.md",
);
const result = JSON.parse(await readFile(file, "utf8"));
assert.equal(result.schema_version, 1);
const runs = result.runs;
const median = (values) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  return sorted.length % 2
    ? sorted[sorted.length >> 1]
    : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
};
const percent = (value) =>
  value === null || !Number.isFinite(value)
    ? "n/a"
    : `${value >= 0 ? "+" : ""}${(value * 100).toFixed(1)}%`;
const number = (value, digits = 1) =>
  Number.isFinite(value) ? value.toFixed(digits) : "n/a";
const cell = (value) =>
  String(value).replaceAll("|", "\\|").replaceAll("\n", " ");
const lines = [
  "# 鉴权性能实验结果",
  "",
  `记录 ${runs.length} 个trial；${runs.filter((run) => !run.validation?.passed).length} 个无效。所有变化按candidate相对baseline计算。`,
  "",
  "| 路由 / 并发 / 候选 / cache TTL / 规模 | 完整对数 | 成功吞吐变化 | P99变化 | 吞吐95%区间 | P99 95%区间 | Go峰值RSS变化 | Rust峰值RSS变化 | 合计峰值RSS变化 |",
  "| --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: |",
];
const comparisons = compareRuns(runs);
for (const group of comparisons) {
  const interval = (value) =>
    value ? value.map(percent).join(" … ") : "不足6对";
  lines.push(
    `| ${cell(group.key)} | ${group.complete_pairs} | ${percent(group.throughput_change_median)} | ${percent(group.p99_change_median)} | ${interval(group.throughput_change_bootstrap_95)} | ${interval(group.p99_change_bootstrap_95)} | ${percent(group.peak_rss_change_median.go)} | ${percent(group.peak_rss_change_median.rust)} | ${percent(group.peak_rss_change_median.combined)} |`,
  );
}
if (comparisons.some((group) => !group.six_valid_pairs))
  lines.push(
    "",
    "存在不足六对或无效trial；这些数字只用于检查工具与发现线索，不能作为已证实的优化收益。",
  );
lines.push(
  "",
  "| 路由 / 角色 | req/s | P99 ms | Go CPU ms/千成功请求 | Rust CPU ms/千成功请求 | Go 峰值→结束 RSS MiB | Rust 峰值→结束 RSS MiB | 失败请求 |",
  "| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |",
);
const groups = new Map();
for (const run of runs) {
  const key = `${comparisonKey(run)}/${run.role}`;
  if (!groups.has(key)) groups.set(key, []);
  groups.get(key).push(run);
}
for (const [key, values] of groups) {
  const med = (read) => median(values.map(read));
  const memory = (name) => {
    const peak = med((run) => run.resources?.[name]?.peak_rss_bytes);
    const end = med((run) => run.resources?.[name]?.end_rss_bytes);
    return `${number(peak === null ? null : peak / 1048576, 2)} → ${number(end === null ? null : end / 1048576, 2)}`;
  };
  lines.push(
    `| ${cell(key)} | ${number(med((run) => run.measurement?.successful_requests_per_second))} | ${number(med((run) => run.measurement?.p99_ms))} | ${number(med((run) => run.resources?.go?.cpu_ms_per_1000_success))} | ${number(med((run) => run.resources?.rust?.cpu_ms_per_1000_success))} | ${memory("go")} | ${memory("rust")} | ${values.reduce((sum, run) => sum + (run.measurement?.failures ?? 0), 0)} |`,
  );
}
const profiled = runs.filter((run) => run.operation_profile);
if (profiled.length) {
  lines.push(
    "",
    "| 路由 / 角色 | executor调用/成功请求 | executor wall ms/成功请求 | executor CPU ms/成功请求 | admission等待 ms/成功请求 | idle executor调用/秒 |",
    "| --- | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const run of profiled) {
    const profile = run.operation_profile;
    lines.push(
      `| ${cell(`${run.candidate}/${run.scenario}/${run.role}/pair${run.pair + 1}`)} | ${number(profile.executor_calls_per_success, 3)} | ${number(profile.executor_wall_ms_per_success, 3)} | ${number(profile.executor_cpu_ms_per_success, 3)} | ${number(profile.admission_wall_ms_per_success, 3)} | ${number(profile.idle_executor_calls_per_second, 3)} |`,
    );
  }
  lines.push(
    "",
    "Operation recorder统计进程内executor job scope，包含同一捕获窗口的背景操作；每成功请求归一化不等于每请求精确SQL条数。一次executor job可能执行多条SQL。idle频率保留作背景参照，不自动扣减。profiling会增加开销，应另跑关闭profiling的性能A/B。",
  );
}
let unavailable = 0;
const queues = [];
const phases = {};
for (const run of runs) {
  let raw = run;
  if (!run.runtime_health) {
    try {
      raw = JSON.parse(
        await readFile(
          run.raw_result_file ?? path.join(run.directory, "result.json"),
          "utf8",
        ),
      );
    } catch {
      unavailable++;
      continue;
    }
  }
  for (const sample of raw.runtime_health ?? [])
    if (sample.storage) queues.push(sample.storage);
  for (const event of raw.timeout_phase_events ?? []) {
    const phase = event.fields?.phase ?? "unknown";
    phases[phase] = (phases[phase] ?? 0) + 1;
  }
}
const max = (field) => {
  const values = queues.map((queue) => queue[field]).filter(Number.isFinite);
  return values.length ? Math.max(...values) : null;
};
lines.push(
  "",
  `SQLite采样最大队列深度 ${number(max("queue_depth"), 0)}，最大队列等待 ${number(max("queue_wait_ms"))}ms，最大活动操作 ${number(max("active_operation_ms"))}ms。`,
  "",
  `超时phase事件：${
    Object.keys(phases).length
      ? Object.entries(phases)
          .map(([phase, count]) => `${cell(phase)}=${count}`)
          .join("，")
      : "未记录"
  }。${unavailable ? `${unavailable}份原始trial文件当前不可读。` : ""}`,
  "",
  "此工具的正常负载结果不等于故障恢复测试。Rust暂停/退出、SQLite写锁和上游断开恢复应单独故障注入并记录恢复截止时间。RSS为负载期间峰值和结束值，没有主动触发GC，也不代表空闲30秒后的保留量。",
  "",
  "grant_renewal是每token仅执行一次的有限批次；短smoke中的CPU tick量化和少量尾延迟样本不适合衡量收益。phase只反映已记录的超时事件，空结果不表示SQLite等待为零。",
);
process.stdout.write(lines.join("\n") + "\n");
