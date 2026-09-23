// Pure local gate contract test. No experiment, process spawn, network or build.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';
const bundle=path.dirname(fileURLToPath(import.meta.url));
const pin=JSON.parse(await readFile(path.join(bundle,'manifest.json'),'utf8'));
const tools=process.argv[2]??'/Users/edgeware/Local/fn-knock-turborepo/scripts';
for(const name of ['check-auth-performance.mjs','auth-performance-lib.mjs']) {
  const hash=createHash('sha256').update(await readFile(path.join(tools,name))).digest('hex');
  assert.equal(hash,pin.tools.files[name],`${name} is not the pinned frozen tool`);
}
const {comparisonFailures}=await import(pathToFileURL(path.join(tools,'auth-performance-lib.mjs')));
const flags={requireSixPairs:true,requireImprovement:true};
const base={key:'synthetic-independent-confirmation',invalid_runs:0,duplicate_runs:0,incomplete_pairs:0,recovery_probe:false,complete_pairs:6,six_valid_pairs:true,rss_complete_pairs:6,throughput_change_median:0.2,throughput_change_bootstrap_95:[0.1,0.3],p99_change_median:-0.2,p99_change_bootstrap_95:[-0.3,-0.1],peak_rss_change_median:{go:0,rust:0,combined:0}};
const cases=[
 ['valid',{},true],
 ['RSS boundary5%',{peak_rss_change_median:{combined:0.05}},true],
 ['RSS above5%',{peak_rss_change_median:{combined:0.050001}},false],
 ['RPS floor-5% with P99 improvement',{throughput_change_median:-0.05},true],
 ['RPS below-5%',{throughput_change_median:-0.050001},false],
 ['P99 ceiling+10% with RPS improvement',{p99_change_median:0.1},true],
 ['P99 above+10%',{p99_change_median:0.100001},false],
 ['missing RSS',{peak_rss_change_median:{combined:null}},false],
 ['missing RSS pair',{rss_complete_pairs:5},false],
 ['five pairs',{complete_pairs:5,rss_complete_pairs:5,six_valid_pairs:false},false],
 ['no improvement',{throughput_change_median:0,p99_change_median:0},false],
 ['RPS target lower CI at zero fails',{throughput_change_median:0.1,throughput_change_bootstrap_95:[0,0.2],p99_change_median:0},false],
 ['RPS target positive CI succeeds',{throughput_change_median:0.1,throughput_change_bootstrap_95:[0.000001,0.2],p99_change_median:0},true],
 ['P99 target upper CI at zero fails',{throughput_change_median:0,p99_change_median:-0.15,p99_change_bootstrap_95:[-0.2,0]},false],
 ['P99 target negative CI succeeds',{throughput_change_median:0,p99_change_median:-0.15,p99_change_bootstrap_95:[-0.2,-0.000001]},true],
 ['invalid trial',{invalid_runs:1},false],
 ['recovery probe',{recovery_probe:true},false],
];
for(const [name,patch,expected] of cases) assert.equal(comparisonFailures([{...base,...patch}],flags).length===0,expected,name);
console.log(JSON.stringify({frozen_hashes_verified:true,pure_synthetic_gate_tests:cases.length,passed:true,no_experiment_executed:true},null,2));
