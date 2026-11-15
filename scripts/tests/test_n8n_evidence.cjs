// Isolated checks only: this never executes an n8n workflow or sends a report.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const path = require('node:path');
const plan = JSON.parse(fs.readFileSync(path.join(__dirname, '../../deployment/n8n-truenas-migration.json')));
const config = JSON.parse(plan.after['Report Configuration'].jsonOutput);
const code = plan.after['Analyze Targets'].jsCode;
const now = Date.now() / 1000;
const base = {job: 'truenas-api-exporter', instance: 'truenas-exporter:9110', ipaddress: '10.10.10.7', target: 'memory-alpha'};
const metric = (name, value, labels = {}) => ({metric: {...base, __name__: name, ...labels}, value: [now, String(value)]});
const good = [metric('up', 1), metric('truenas_api_connected', 1), metric('truenas_system_uptime_seconds', 8200000),
  metric('truenas_exporter_collection_success', 1, {collector: 'system'}),
  metric('truenas_exporter_snapshot_age_seconds', 40, {collector: 'system'}),
  metric('truenas_exporter_last_success_timestamp_seconds', now - 40, {collector: 'system'})];
const run = (rows, status = 'success', source = code, cfg = config) => vm.runInNewContext('(function(){' + source + '})()', {
  Date, $: () => ({first: () => ({json: cfg})}), $json: {status, data: {result: rows}},
})[0].json;
let passed = 0;
function check(name, rows, state = 'unknown', status = 'success') {
  const result = run(rows, status);
  assert.equal(result.criticalSystems.find(x => x.ip === '10.10.10.7').state, state, name);
  assert.equal(result.total, rows.filter(x => x.metric.__name__ === 'up').length);
  passed++;
}
const change = (name, value) => good.map(x => x.metric.__name__ === name ? {...x, value: [now, String(value)]} : x);
check('healthy', good, 'healthy');
for (const row of good) check('missing ' + row.metric.__name__, good.filter(x => x !== row));
for (const name of ['up', 'truenas_api_connected', 'truenas_exporter_collection_success']) check('zero ' + name, change(name, 0));
check('expired snapshot', change('truenas_exporter_snapshot_age_seconds', 181));
check('expired success timestamp', change('truenas_exporter_last_success_timestamp_seconds', now - 181));
check('duplicate', good.concat(good[1]));
check('wrong target', good.map(x => x.metric.__name__ === 'up' ? x : {...x, metric: {...x.metric, target: 'other'}}));
check('wrong collector', good.map(x => x.metric.collector ? {...x, metric: {...x.metric, collector: 'cpu'}} : x));
check('stale scrape timestamps', good.map(x => ({...x, value: [now - 301, x.value[1]]})));
check('unsuccessful query', [], 'unknown', 'error');
check('invalid uptime', change('truenas_system_uptime_seconds', 'NaN'));
check('future success', change('truenas_exporter_last_success_timestamp_seconds', now + 31));
const ordinary = metric('up', 1, {job: 'node_exporter-10.5', instance: '10.10.10.5:9100', target: undefined});
assert.equal(run([ordinary]).criticalSystems.find(x => x.ip === '10.10.10.5').state, 'healthy');
passed++;
// The migration must preserve availability/count behavior for unrelated hosts.
const oldConfig = JSON.parse(plan.before['Report Configuration'].jsonOutput);
const oldCode = plan.before['Analyze Targets'].jsCode;
const previous = run([ordinary], 'success', oldCode, oldConfig);
const current = run([ordinary]);
assert.equal(JSON.stringify(current.criticalSystems.filter(x => x.ip !== '10.10.10.7')),
  JSON.stringify(previous.criticalSystems.filter(x => x.ip !== '10.10.10.7')));
assert.equal(current.total, previous.total);
assert.equal(current.up, previous.up);
passed++;
console.log(`${passed} isolated n8n evidence cases passed; no workflow execution or report sent`);
