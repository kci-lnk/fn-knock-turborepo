// Pure synthetic frozen-library check, without services or benchmark load.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
const root=path.dirname(fileURLToPath(import.meta.url));
const m=JSON.parse(await readFile(path.join(root,'manifest.json'),'utf8'));
const tools=process.argv[2]??'/tmp/fn-knock-auth-tools5-20260923';
for(const name of ['check-auth-performance.mjs','auth-performance-lib.mjs'])assert.equal(createHash('sha256').update(await readFile(path.join(tools,name))).digest('hex'),m.tools.files[name]);
const {comparisonFailures}=await import(pathToFileURL(path.join(tools,'auth-performance-lib.mjs')));
const base={key:'synthetic-audit-fix',invalid_runs:0,duplicate_runs:0,incomplete_pairs:0,recovery_probe:false,complete_pairs:6,six_valid_pairs:true,rss_complete_pairs:6,throughput_change_median:0,p99_change_median:0,peak_rss_change_median:{combined:0}};
const cases=[['no improvement',{},true],['RPS floor',{throughput_change_median:-.05},true],['RPS failure',{throughput_change_median:-.050001},false],['P99 ceiling',{p99_change_median:.1},true],['P99 failure',{p99_change_median:.100001},false],['RSS ceiling',{peak_rss_change_median:{combined:.05}},true],['RSS failure',{peak_rss_change_median:{combined:.050001}},false],['missing RSS',{peak_rss_change_median:{combined:null}},false],['five pairs',{complete_pairs:5,six_valid_pairs:false,rss_complete_pairs:5},false],['invalid',{invalid_runs:1},false],['recovery strict rejected',{recovery_probe:true},false]];
for(const [name,patch,pass] of cases)assert.equal(comparisonFailures([{...base,...patch}],{requireSixPairs:true,requireImprovement:false}).length===0,pass,name);
console.log(JSON.stringify({synthetic_contract_only:true,frozen_hashes_verified:true,gate_cases:cases.length,passed:true,experiment_executed:false},null,2));
