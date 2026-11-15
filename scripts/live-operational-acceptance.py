#!/usr/bin/env python3
"""Temporary, bounded acceptance harness; never a deployed monitoring service."""
from __future__ import annotations

import argparse
import hashlib
import http.client
import ipaddress
import json
import math
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from urllib.parse import urlencode
from zoneinfo import ZoneInfo

IMAGE = 'sha256:c8f69fc12a105a740e473e7b31a08d00ad06a5fa099263ac446c15efe79551b4'
UP = 'up{job="truenas-api-exporter",instance="truenas-exporter:9110"}'
GROUPS = ('pools', 'system', 'cpu', 'memory', 'network', 'arc')
FLAGS = {'truenas_api_connected': 1, 'truenas_api_compatible': 1,
         'truenas_realtime_subscribed': 1, 'truenas_exporter_mock_mode': 0}
OP = ('truenas_exporter_collection_success', 'truenas_exporter_snapshot_age_seconds',
      'truenas_exporter_last_success_timestamp_seconds')
MEASUREMENTS = {
    'truenas_pool_size_bytes': {'pool': 'tank'},
    'truenas_system_uptime_seconds': {},
    'truenas_cpu_utilization_ratio': {'cpu': 'cpu'},
    'truenas_memory_total_bytes': {}, 'truenas_memory_available_bytes': {},
    'truenas_network_receive_bytes_per_second': {'interface': 'enp42s0'},
    'truenas_network_transmit_bytes_per_second': {'interface': 'enp42s0'},
    'truenas_zfs_arc_size_bytes': {},
}
LABEL = re.compile(r'([a-zA-Z_][a-zA-Z_0-9]*)="((?:[^"\\]|\\.)*)"(?:,|$)')


class Failure(Exception):
    pass


def require(condition, reason):
    if not condition:
        raise Failure(reason)


def utc():
    return datetime.now(timezone.utc).isoformat()


def save(path, value):
    temporary = Path(str(path) + '.tmp')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
    temporary.replace(path)


def command(args, seconds=3, limit=65536):
    """Bound command duration and output before accumulation; do not print raw errors."""
    proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    end = time.monotonic() + seconds
    chunks = {proc.stdout.fileno(): bytearray(), proc.stderr.fileno(): bytearray()}
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(proc.stdout, selectors.EVENT_READ)
            selector.register(proc.stderr, selectors.EVENT_READ)
            while selector.get_map():
                remaining = end - time.monotonic()
                require(remaining > 0, 'command deadline exceeded: ' + args[0])
                for key, _ in selector.select(remaining):
                    part = os.read(key.fd, 8192)
                    if not part:
                        selector.unregister(key.fileobj)
                    else:
                        chunks[key.fd].extend(part)
                        require(sum(map(len, chunks.values())) <= limit, 'command output limit')
            code = proc.wait(timeout=max(.01, end - time.monotonic()))
        require(code == 0, 'command failed: ' + args[0] + ' exit ' + str(code))
        return bytes(chunks[proc.stdout.fileno()]).decode('utf-8')
    except subprocess.TimeoutExpired as error:
        raise Failure('command deadline exceeded: ' + args[0]) from error
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait(timeout=2)
        proc.stdout.close()
        proc.stderr.close()


def inspect(cid, docker='docker', seconds=3):
    return json.loads(command([docker, 'inspect', cid], seconds))[0]


def run_guard(cid, folder, docker='docker', delay=120):
    """Called in a detached process. Test injection never reaches live CLI options."""
    require(bool(re.fullmatch('[0-9a-f]{64}', cid)), 'invalid guard container ID')
    folder = Path(folder)
    save(folder / 'guard.ready', {'pid': os.getpid(), 'container': cid, 'utc': utc()})
    time.sleep(delay)
    result = {'container': cid, 'utc': utc()}
    try:
        running = inspect(cid, docker, 5)['State']['Running']
    except (Failure, KeyError, ValueError, OSError):
        running = False
    if running:
        result['result'] = 'already-running'
    else:
        try:
            command([docker, 'start', cid], 10)
            result['result'] = 'restore-start-succeeded'
        except (Failure, OSError):
            result['result'] = 'restore-start-failed'
    save(folder / 'guard.result', result)
    return result


def parse_metrics(text):
    require(text.rstrip().endswith('# EOF'), 'incomplete OpenMetrics response')
    wanted = set(FLAGS) | set(OP) | set(MEASUREMENTS)
    result = {}
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        match = re.fullmatch(r'([a-zA-Z_:][a-zA-Z_0-9:]*)(?:\{(.*)\})? ([^ ]+)', line)
        require(match is not None, 'malformed metric sample')
        name, label_text, number = match.groups()
        if name not in wanted:
            continue
        labels, position = {}, 0
        for item in LABEL.finditer(label_text or ''):
            require(item.start() == position and item[1] not in labels, 'ambiguous metric labels')
            labels[item[1]] = json.loads('"' + item[2] + '"')
            position = item.end()
        require(position == len(label_text or ''), 'invalid metric labels')
        value = float(number)
        require(math.isfinite(value), 'nonfinite measurement')
        if labels.get('target') == 'memory-alpha':
            result.setdefault(name, []).append({'labels': labels, 'value': value})
    return result


def one(metrics, name, labels=None):
    labels = labels or {}
    rows = [row for row in metrics.get(name, [])
            if all(row['labels'].get(key) == value for key, value in labels.items())]
    require(len(rows) == 1, 'missing/duplicate ' + name + ' ' + str(labels))
    return rows[0]['value']


def healthy(observation, started_after=0, baseline=None):
    try:
        require(observation['healthz'] == 200 and observation['readyz'] == 200, 'HTTP not ready')
        require(observation['running'] and observation['container_health'] == 'healthy'
                and not observation['oom'], 'container not healthy')
        m = observation['metrics']
        for name, value in FLAGS.items():
            require(one(m, name) == value, name + ' not ready')
        times = {}
        for group in GROUPS:
            labels = {'collector': group}
            require(one(m, OP[0], labels) == 1, group + ' unsuccessful')
            age = one(m, OP[1], labels)
            last = one(m, OP[2], labels)
            require(0 <= age < (180 if group in ('pools', 'system') else 6), group + ' stale')
            require(started_after < last <= observation['wall'] + observation.get('clock_uncertainty', 2), group + ' old/future timestamp')
            if baseline:
                require(last > baseline[group], group + ' did not advance')
            times[group] = last
        for name, labels in MEASUREMENTS.items():
            require(one(m, name, labels) >= 0, 'invalid ' + name)
        return times
    except Failure:
        return None


class LiveBackend:
    def __init__(self, inputs, folder):
        self.inputs, self.folder = inputs, Path(folder)
        self.cid, self.ip = None, None
        self.end = time.monotonic() + 450
        self.hard_end = self.end + 30
        self.counts = {'direct': 0, 'prometheus': 0}
        self.prom, self.prom_at = None, -math.inf
        self.guard = None

    def clock(self):
        return time.monotonic()

    def sleep(self, seconds):
        time.sleep(max(0, seconds))

    def cmd(self, args, seconds=3, limit=65536):
        self.check_time()
        return command(args, min(seconds, self.end - self.clock()), limit)

    def raw(self, name):
        return json.loads(self.cmd(['docker', 'inspect', name]))[0]

    def check_time(self):
        require(self.clock() < self.end, 'controller deadline')

    def http(self, host, port, path, kind, cap, seconds):
        self.check_time()
        self.counts[kind] += 1
        require(self.counts[kind] <= (288 if kind == 'direct' else 112), kind + ' request cap')
        end = min(self.end, self.clock() + seconds)
        conn = http.client.HTTPConnection(host, port, timeout=max(.01, end - self.clock()))
        try:
            conn.request('GET', path, headers={'Connection': 'close'})
            sock = conn.sock
            sock.settimeout(max(.01, end - self.clock()))
            response = conn.getresponse()
            body = bytearray()
            while True:
                require(self.clock() < end, 'HTTP total deadline')
                sock.settimeout(max(.01, end - self.clock()))
                part = response.read1(min(8192, cap + 1 - len(body)))
                body.extend(part)
                require(len(body) <= cap, 'HTTP body limit')
                if not part or response.isclosed():
                    break
            require(response.length in (None, 0), 'incomplete HTTP body')
            return response.status, bytes(body).decode('utf-8')
        except (OSError, http.client.HTTPException):
            return None, ''
        finally:
            conn.close()

    def query(self, expression, evaluation=None):
        args = {'query': expression}
        if evaluation is not None:
            args['time'] = str(evaluation)
        code, text = self.http('127.0.0.1', 9090, '/api/v1/query?' + urlencode(args),
                               'prometheus', 262144, 3)
        require(code == 200, 'Prometheus query unavailable')
        result = json.loads(text)
        require(result.get('status') == 'success' and result['data']['resultType'] == 'vector',
                'invalid Prometheus result')
        return result['data']['result']

    def targets(self):
        code, text = self.http('127.0.0.1', 9090, '/api/v1/targets', 'prometheus', 262144, 3)
        require(code == 200, 'Prometheus targets unavailable')
        rows = json.loads(text)['data']['activeTargets']
        result = sorted([{'job': r['labels']['job'], 'instance': r['labels']['instance'],
                          'health': r['health']} for r in rows], key=lambda r: (r['job'], r['instance']))
        require(not any(r['job'] == 'truenas-graphite-10.60' for r in result), 'Graphite target exists')
        require(sum(r['job'] == 'truenas-api-exporter' and r['instance'] == 'truenas-exporter:9110'
                    for r in result) == 1, 'ambiguous exporter target')
        return result

    def dashboard_queries(self):
        output = []
        for expression in self.inputs['expressions']:
            rows = self.query(expression)
            require(len(rows) == 1, 'dashboard missing/duplicate series')
            value = float(rows[0]['value'][1])
            require(math.isfinite(value), 'invalid dashboard measurement')
            if 'truenas_api_connected' in expression:
                require(value == 1, 'dashboard status not green')
            output.append({'sha256': hashlib.sha256(expression.encode()).hexdigest(), 'value': value})
        return output

    def metadata(self, raw):
        config, host = raw['Config'], raw['HostConfig']
        fields = ['TRUENAS_TARGET', 'TRUENAS_SYSTEM_ENABLED', 'TRUENAS_REALTIME_ENABLED',
                  'TRUENAS_REALTIME_INTERVAL_SECONDS', 'TRUENAS_REALTIME_TTL_SECONDS',
                  'TRUENAS_POLL_INTERVAL_SECONDS', 'TRUENAS_API_TIMEOUT_SECONDS',
                  'TRUENAS_CACHE_TTL_SECONDS', 'EXPORTER_LISTEN', 'EXPORTER_MOCK']
        env = dict(item.split('=', 1) for item in config['Env'] if '=' in item)
        settings = {key: env.get(key) for key in fields}
        require(settings['TRUENAS_TARGET'] == 'memory-alpha' and
                settings['TRUENAS_SYSTEM_ENABLED'] == settings['TRUENAS_REALTIME_ENABLED'] == 'true' and
                settings['TRUENAS_REALTIME_INTERVAL_SECONDS'] == '2' and
                settings['TRUENAS_POLL_INTERVAL_SECONDS'] == '60' and
                settings['TRUENAS_CACHE_TTL_SECONDS'] == '180' and
                settings['EXPORTER_LISTEN'] == '0.0.0.0:9110' and
                settings['TRUENAS_REALTIME_TTL_SECONDS'] in (None, '6') and
                settings['TRUENAS_API_TIMEOUT_SECONDS'] in (None, '10') and
                settings['EXPORTER_MOCK'] in (None, 'false'), 'collection settings drift')
        require(raw['Image'] == IMAGE and config['Image'] == 'local/truenas-rust-exporter:0.8.0',
                'deployed image drift')
        require(config['User'] == '10001:10001' and host['ReadonlyRootfs'] and host['Init'] and
                host['CapDrop'] == ['ALL'] and 'no-new-privileges:true' in host['SecurityOpt'] and
                host['Memory'] == 134217728 and host['NanoCpus'] == 500000000 and
                host['PidsLimit'] == 64 and host['RestartPolicy']['Name'] == 'unless-stopped',
                'container hardening drift')
        require(config.get('StopSignal') in (None, '', 'SIGTERM', '15'), 'unexpected stop signal')
        labels = config['Labels']
        require(labels.get('com.docker.compose.project') == 'observability' and
                labels.get('com.docker.compose.service') == 'truenas-exporter', 'wrong Compose service')
        require(set(raw['NetworkSettings']['Networks']) == {'observability'}, 'network drift')
        require(not any(raw['NetworkSettings']['Ports'].values()), 'unexpected published port')
        mounts = sorted((m['Source'], m['Destination'], m['RW']) for m in raw['Mounts'])
        expected = self.inputs['expected']['services']['truenas-exporter']['volumes']
        require(mounts == sorted((x.split(':')[0], x.split(':')[1], False) for x in expected),
                'mount metadata drift')
        return {'container': raw['Id'], 'image': raw['Image'], 'settings': settings,
                'mounts': mounts, 'restart_count': raw['RestartCount']}

    def surrounding(self):
        services = {}
        for name in ('prometheus', 'grafana', 'grafana-mcp', 'loki'):
            raw = self.raw(name)
            services[name] = {'id': raw['Id'], 'started': raw['State']['StartedAt'],
                              'running': raw['State']['Running'],
                              'health': raw['State'].get('Health', {}).get('Status')}
            require(services[name]['running'], 'surrounding service stopped')
        hashes = {}
        for name in ('docker-compose.yml', 'prometheus-config.yml'):
            path = Path('/mnt/observability-stack') / name
            hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        return {'services': services, 'config_sha256': hashes}

    def preflight(self):
        require(os.uname().nodename.split('.')[0] == 'observability-stack', 'wrong Docker host')
        require(abs(self.inputs['clock_offset_seconds']) <= 2 and
                self.inputs['clock_roundtrip_seconds'] <= 2, 'clock agreement not established')
        require('--timeout' in self.cmd(['docker', 'stop', '--help']), 'unsupported stop CLI')
        raw = self.raw('truenas-exporter')
        self.cid = raw['Id']
        require(bool(re.fullmatch('[0-9a-f]{64}', self.cid)), 'invalid container ID')
        self.initial = self.metadata(raw)
        require(self.cmd(['docker', 'image', 'inspect', IMAGE, '--format', '{{.Id}}']).strip() == IMAGE, 'pinned image missing')
        require(raw['State']['Running'] and not raw['State']['OOMKilled'], 'unhealthy initial container')
        self.ip = raw['NetworkSettings']['Networks']['observability']['IPAddress']
        require(ipaddress.ip_address(self.ip).is_private, 'unexpected container address')
        require(not self.cmd(['docker', 'ps', '-a', '--filter', 'name=^/graphite-exporter$',
                             '--format', '{{.ID}}']).strip(), 'Graphite container exists')
        return {'exporter': self.initial, 'surrounding': self.surrounding(),
                'clock': {k: self.inputs[k] for k in ('clock_offset_seconds', 'clock_roundtrip_seconds', 'clock_uncertainty_seconds')},
                'targets': self.targets()}

    def observe(self):
        raw = self.raw(self.cid)
        require(raw['Image'] == IMAGE and raw['RestartCount'] == self.initial['restart_count'],
                'unexpected image or auto-restart')
        codes, metrics = {}, {}
        direct_deadline = self.clock() + 2
        for endpoint in ('healthz', 'readyz', 'metrics'):
            code, body = self.http(self.ip, 9110, '/' + endpoint, 'direct',
                                   262144 if endpoint == 'metrics' else 4096,
                                   min(2, max(.01, direct_deadline - self.clock())))
            codes[endpoint] = code
            if endpoint == 'metrics' and code == 200:
                metrics = parse_metrics(body)
        if self.clock() - self.prom_at >= 10:
            at = time.time()
            rows, stamp = self.query(UP, at), self.query('timestamp(' + UP + ')', at)
            require(len(rows) == len(stamp) == 1, 'missing/duplicate scrape evidence')
            self.prom = {'up': float(rows[0]['value'][1]), 'sample': float(stamp[0]['value'][1])}
            require(self.prom['up'] in (0, 1) and math.isfinite(self.prom['sample']), 'invalid scrape')
            self.prom_at = self.clock()
        return {'wall': time.time(), 'clock_uncertainty': self.inputs['clock_uncertainty_seconds'], 'utc': utc(), 'running': raw['State']['Running'],
                'container_health': raw['State'].get('Health', {}).get('Status'),
                'oom': raw['State']['OOMKilled'], 'healthz': codes['healthz'],
                'readyz': codes['readyz'], 'metrics_http': codes['metrics'], 'metrics': metrics, 'prom': self.prom}

    def arm(self):
        require(not (self.folder / 'guard.ready').exists(), 'guard already armed')
        with (self.folder / 'guard.log').open('wb') as log:
            self.guard = subprocess.Popen(['timeout', '--signal=TERM', '--kill-after=2s', '148s',
                sys.executable, str(Path(__file__).resolve()), '--guard', self.cid, str(self.folder)],
                stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
        end = self.clock() + 5
        while not (self.folder / 'guard.ready').exists():
            require(self.clock() < end and self.guard.poll() is None, 'guard failed to arm')
            self.sleep(.05)
        require(json.loads((self.folder / 'guard.ready').read_text())['container'] == self.cid,
                'guard wrong container')

    def stop(self):
        raw = self.raw(self.cid)
        require(self.metadata(raw) == self.initial and raw['State']['Running'], 'pre-stop drift')
        begin = utc()
        t = self.clock()
        self.cmd(['docker', 'stop', '--timeout', '10', self.cid], 15)
        raw = self.raw(self.cid)
        end = utc()
        require(not raw['State']['Running'] and raw['State']['ExitCode'] == 0 and
                not raw['State']['OOMKilled'], 'non-graceful exit')
        events = self.cmd(['docker', 'events', '--since', begin, '--until', end,
            '--filter', 'type=container', '--filter', 'container=' + self.cid,
            '--filter', 'event=kill', '--filter', 'event=die', '--filter', 'event=stop',
            '--format', '{{.TimeNano}} {{.Action}} {{index .Actor.Attributes "signal"}} {{index .Actor.Attributes "exitCode"}}'], 3, 16384).splitlines()
        require(0 < len(events) <= 64 and any(' die ' in row for row in events), 'missing lifecycle evidence')
        require(any(' kill 15' in row or ' kill SIGTERM' in row for row in events), 'SIGTERM not observed')
        require(not any(' kill 9' in row or ' kill SIGKILL' in row for row in events), 'forced termination')
        return {'duration': self.clock() - t, 'completed': time.time(), 'exit_code': 0, 'events': events}

    def start(self):
        self.cmd(['docker', 'start', self.cid], 10)
        raw = self.raw(self.cid)
        require(self.metadata(raw) == self.initial, 'post-start identity/settings drift')
        self.ip = raw['NetworkSettings']['Networks']['observability']['IPAddress']
        return datetime.fromisoformat(raw['State']['StartedAt'].replace('Z', '+00:00')).timestamp()

    def finish(self, initial):
        require(self.metadata(self.raw(self.cid)) == self.initial, 'final exporter drift')
        require(self.surrounding() == initial['surrounding'], 'surrounding services/configuration changed')
        targets = self.targets()
        require(targets == initial['targets'], 'target inventory/health changed')
        require((self.folder / 'guard.result').exists(), 'guard completion missing')
        guard = json.loads((self.folder / 'guard.result').read_text())
        require(guard['result'] == 'already-running', 'guard-assisted restoration is not a pass')
        return {'guard': guard, 'targets': targets, 'queries': self.dashboard_queries()}

    def restore(self):
        self.end = min(self.hard_end, self.clock() + 25)
        result = {}
        if self.cid:
            try:
                if not self.raw(self.cid)['State']['Running']:
                    self.cmd(['docker', 'start', self.cid], 10)
                    result['fallback_start'] = True
                raw = self.raw(self.cid)
                result['running'] = raw['State']['Running']
                if result['running']:
                    self.ip = raw['NetworkSettings']['Networks']['observability']['IPAddress']
                    for name in ('healthz', 'readyz'):
                        result[name] = self.http(self.ip, 9110, '/' + name, 'direct', 4096, 2)[0]
            except Failure:
                result['running'] = 'unverified'
        if (self.folder / 'guard.result').exists():
            result['guard'] = json.loads((self.folder / 'guard.result').read_text())
        elif self.guard:
            result['guard'] = {'status': 'pending', 'pid': self.guard.pid, 'maximum_lifetime_seconds': 150}
        return result


def run(backend, folder):
    """Same state machine is exercised by synthetic rehearsals and live execution."""
    start, mutated = backend.clock(), False
    report = {'status': 'RUNNING', 'utc_start': utc(), 'operator_date':
              datetime.now(ZoneInfo('America/Chicago')).date().isoformat(),
              'plan_commit': 'e2575bf', 'controller_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'phases': [], 'observations': []}
    normal_end = start + 450

    def phase(name):
        limits = {'preflight': 20, 'baseline': 60, 'stop': 25, 'stopped-observation': 70,
                  'start': 15, 'recovery': 150, 'stable-recovery': 80, 'final': 20}
        backend.end = min(normal_end, backend.clock() + limits[name])
        require(backend.clock() < normal_end, 'normal-work deadline')
        report['phases'].append({'name': name, 'elapsed': backend.clock() - start})
        save(Path(folder) / 'result.json', report)
        print(json.dumps({'phase': name, 'elapsed': round(backend.clock() - start, 2)}), flush=True)

    def sample():
        require(backend.clock() < normal_end, 'normal-work deadline')
        obs = backend.observe()
        report['observations'].append(obs)
        return obs

    def wait_period(seconds, validate):
        until = backend.clock() + seconds
        last = None
        while backend.clock() < until:
            tick = backend.clock()
            last = sample()
            validate(last)
            backend.sleep(max(0, min(until, tick + 5) - backend.clock()))
        return last

    try:
        phase('preflight')
        t = backend.clock()
        initial = backend.preflight()
        report['baseline_metadata'] = initial
        require(backend.clock() - t <= 20, 'preflight phase deadline')
        phase('baseline')
        def baseline_gate(o):
            require(healthy(o) is not None, 'baseline not healthy/fresh')
            require(o['prom']['up'] == 1 and o['wall'] - o['prom']['sample'] <= 45,
                    'baseline scrape unavailable/stale')
        baseline_begin = backend.clock()
        report['baseline_queries'] = backend.dashboard_queries()
        require(backend.clock() - baseline_begin < 60, 'baseline query deadline')
        last = wait_period(60 - (backend.clock() - baseline_begin), baseline_gate)
        baseline = healthy(last)
        phase('stop')
        t = backend.clock()
        backend.arm()
        mutated = True
        stopped = backend.stop()
        require(backend.clock() - t <= 25, 'stop phase deadline')
        report['shutdown'] = stopped
        phase('stopped-observation')
        saw_down = []
        def down_gate(o):
            require(not o['running'] and o['healthz'] is None and o['readyz'] is None and o['metrics_http'] is None,
                    'exporter did not remain stopped')
            if o['prom']['up'] == 0 and o['prom']['sample'] > stopped['completed']:
                saw_down.append(o['prom'])
        wait_period(70, down_gate)
        require(saw_down, 'fresh down scrape not observed')
        report['down_scrape'] = saw_down[-1]
        phase('start')
        t = backend.clock()
        restarted = backend.start()
        require(backend.clock() - t <= 15, 'start phase deadline')
        report['new_started_at'] = restarted
        phase('recovery')
        end = backend.clock() + 150
        recovery = None
        while backend.clock() < end:
            tick = backend.clock()
            obs = sample()
            current = healthy(obs, restarted, baseline)
            if current and obs['prom']['up'] == 1 and obs['prom']['sample'] > max(restarted, saw_down[-1]['sample']) and obs['wall'] - obs['prom']['sample'] <= 45:
                recovery = current
                break
            backend.sleep(min(5, max(0, end - backend.clock())))
        require(recovery, 'recovery deadline')
        report['recovered_success_times'] = recovery
        phase('stable-recovery')
        def stable_gate(o):
            require(healthy(o, restarted) is not None and o['prom']['up'] == 1 and
                    o['wall'] - o['prom']['sample'] <= 45, 'stable recovery broken')
        final = wait_period(80, stable_gate)
        require(healthy(final, restarted, recovery), 'stable snapshots did not advance')
        phase('final')
        t = backend.clock()
        final_observation = sample()
        require(healthy(final_observation, restarted) is not None, 'final readiness/freshness failed')
        report['final_success_times'] = healthy(final_observation, restarted)
        report['final'] = backend.finish(initial)
        require(backend.clock() - t <= 20, 'final phase deadline')
        report['status'] = 'PASS'
    except (Failure, OSError, ValueError, KeyError) as error:
        report['status'] = 'FAIL'
        report['reason'] = str(error) if isinstance(error, Failure) else type(error).__name__
    finally:
        if mutated and report['status'] != 'PASS':
            report['restoration'] = backend.restore()
        report['elapsed'] = backend.clock() - start
        report['utc_end'] = utc()
        report['request_counts'] = backend.counts
        if report['elapsed'] > 480:
            report['status'] = 'FAIL'
            report['reason'] = 'global window exceeded'
        save(Path(folder) / 'result.json', report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--guard', nargs=2, metavar=('CONTAINER', 'DIRECTORY'))
    parser.add_argument('--execute-plan-e2575bf', action='store_true')
    parser.add_argument('--input', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.guard:
        run_guard(*args.guard)
        return
    require(args.execute_plan_e2575bf and args.input and args.output, 'explicit plan execution required')
    require(sys.platform.startswith('linux'), 'live controller requires Linux host')
    os.umask(0o077)
    require(not (args.output / 'result.json').exists(), 'window already attempted; no automatic rerun')
    args.output.mkdir(mode=0o700, parents=False, exist_ok=True)
    require(args.output.stat().st_mode & 0o077 == 0, 'task directory must be private')
    inputs = json.loads(args.input.read_text())
    require(len(inputs['expressions']) == 7 and len(inputs['panels']) == 12, 'invalid dashboard snapshot')
    # SSH transport clock check is performed before the window; run uses this
    # host's clocks thereafter. Do not copy a Mac monotonic timestamp to Linux.
    require(abs(time.time() - inputs['host_epoch']) <= 10, 'stale clock handshake')
    inputs['controller_epoch'] = time.time()
    inputs['controller_monotonic_on_host'] = time.monotonic()
    backend = LiveBackend(inputs, args.output)
    def interrupt(_signal, _frame):
        raise Failure('controller interrupted')
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, interrupt)
    result = run(backend, args.output)
    print(json.dumps({'status': result['status'], 'elapsed': result['elapsed'],
                      'reason': result.get('reason')}), flush=True)
    raise SystemExit(0 if result['status'] == 'PASS' else 1)


if __name__ == '__main__':
    main()
