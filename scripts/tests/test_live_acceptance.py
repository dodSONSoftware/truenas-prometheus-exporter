"""Synthetic rehearsal of the actual acceptance state machine and guard."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('live_acceptance', ROOT / 'scripts/live-operational-acceptance.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class SyntheticBackend:
    def __init__(self, fault=None):
        self.t = 0
        self.started = -120
        self.running = True
        self.fault = fault
        self.actions = []
        self.counts = {'direct': 0, 'prometheus': 0}
        self.prom_at = -100
        self.prom = None
        self.end = 450

    def clock(self):
        return self.t

    def sleep(self, seconds):
        self.t += seconds

    def preflight(self):
        if self.fault == 'baseline':
            raise M.Failure('wrong image')
        if self.fault == 'preflight_timeout':
            self.t += 21
        self.counts['prometheus'] += 1
        return {'surrounding': {}, 'targets': []}

    def dashboard_queries(self):
        self.counts['prometheus'] += 7
        return [{'value': 1}] * 7

    def observe(self):
        self.counts['direct'] += 3
        ready = self.running and self.t - self.started >= 20
        if self.fault == 'stable' and self.started > 0 and self.t - self.started >= 60:
            ready = False
        metrics = {}
        def add(name, value, labels=None):
            metrics.setdefault(name, []).append({'labels': {'target': 'memory-alpha', **(labels or {})}, 'value': value})
        if self.running:
            for name, value in M.FLAGS.items():
                add(name, value)
            for group in M.GROUPS:
                interval = 60 if group in ('pools', 'system') else 2
                last = self.started + 1 + int(max(0, self.t - self.started - 1) // interval) * interval
                if self.fault == 'stale' and self.started > 0:
                    last = 0
                add(M.OP[0], 1, {'collector': group})
                add(M.OP[1], max(0, self.t - last), {'collector': group})
                add(M.OP[2], 1000000 + last, {'collector': group})
            for name, labels in M.MEASUREMENTS.items():
                add(name, 1, labels)
        if self.t - self.prom_at >= 10:
            self.counts['prometheus'] += 2
            self.prom = {'up': int(self.running or self.fault == 'no_down'),
                         'sample': 1000000 + int(self.t // 30) * 30}
            self.prom_at = self.t
        return {'wall': 1000000 + self.t, 'utc': 'synthetic', 'running': self.running,
                'container_health': 'healthy' if ready else 'starting', 'oom': False,
                'healthz': 200 if self.running else None,
                'readyz': (200 if ready else 503) if self.running else None,
                'metrics_http': 200 if self.running else None, 'metrics': metrics,
                'prom': self.prom}

    def arm(self):
        self.actions.append('arm')

    def stop(self):
        self.actions.append('stop')
        self.running = False
        if self.fault == 'stop':
            raise M.Failure('non-graceful exit')
        if self.fault == 'interrupt':
            raise M.Failure('controller interrupted')
        return {'completed': 1000000 + self.t, 'exit_code': 0}

    def start(self):
        self.actions.append('start')
        self.running = True
        self.started = self.t
        return 1000000 + self.started

    def finish(self, _initial):
        if self.fault == 'surrounding':
            raise M.Failure('surrounding services/configuration changed')
        if self.fault == 'guard':
            raise M.Failure('guard-assisted restoration is not a pass')
        self.counts['prometheus'] += 1
        return {'queries': self.dashboard_queries(), 'guard': 'already-running'}

    def restore(self):
        self.actions.append('restore')
        self.running = True
        return {'running': True}


class LiveAcceptanceTests(unittest.TestCase):
    def test_synthetic_full_window_and_request_caps(self):
        with tempfile.TemporaryDirectory() as directory:
            backend = SyntheticBackend()
            report = M.run(backend, directory)
            self.assertEqual(report['status'], 'PASS')
            self.assertEqual(backend.actions, ['arm', 'stop', 'start'])
            self.assertLess(report['elapsed'], 450)
            self.assertLessEqual(backend.counts['direct'], 288)
            self.assertLessEqual(backend.counts['prometheus'], 112)
            self.assertGreater(report['recovered_success_times']['pools'], report['new_started_at'])

    def test_preflight_failures_never_stop(self):
        for fault in ['baseline', 'preflight_timeout']:
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as directory:
                backend = SyntheticBackend(fault)
                self.assertEqual(M.run(backend, directory)['status'], 'FAIL')
                self.assertEqual(backend.actions, [])

    def test_failures_restore_without_repeating_test(self):
        for fault in ['stop', 'interrupt', 'no_down', 'stale', 'stable', 'surrounding', 'guard']:
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as directory:
                backend = SyntheticBackend(fault)
                report = M.run(backend, directory)
                self.assertEqual(report['status'], 'FAIL')
                self.assertEqual(backend.actions.count('stop'), 1)
                self.assertLessEqual(backend.actions.count('start'), 1)
                self.assertEqual(backend.actions.count('restore'), 1)
                self.assertTrue(backend.running)
                self.assertLess(report['elapsed'], 480)

    def test_guard_survives_parent_exit_and_starts_only_stopped_target(self):
        for running, failed in [(False, False), (True, False), (False, True)]:
            with self.subTest(running=running, failed=failed), tempfile.TemporaryDirectory() as directory:
                folder = Path(directory)
                (folder / 'state').write_text('true' if running else 'false')
                if failed:
                    (folder / 'fail').touch()
                stub = folder / 'docker-stub'
                stub.write_text('#!' + sys.executable + '\n' + '''import json,sys
from pathlib import Path
p=Path(__file__).parent
if sys.argv[1]=='inspect':
 print(json.dumps([{'State':{'Running':(p/'state').read_text()=='true'}}]))
elif sys.argv[1]=='start':
 assert sys.argv[2]=='a'*64
 with (p/'calls').open('a') as f: f.write('start\\n')
 if (p/'fail').exists(): sys.exit(1)
 (p/'state').write_text('true')
else: sys.exit(2)
''')
                stub.chmod(0o700)
                module = str(ROOT / 'scripts/live-operational-acceptance.py')
                guard_code = ('import importlib.util; s=importlib.util.spec_from_file_location("guard",' +
                              repr(module) + '); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); '+
                              'm.run_guard(' + repr('a'*64) + ',' + repr(str(folder)) + ',' + repr(str(stub)) + ',.2)')
                parent = ('import subprocess,sys; subprocess.Popen([sys.executable,"-c",' + repr(guard_code) +
                          '],start_new_session=True,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)')
                subprocess.run([sys.executable, '-c', parent], check=True, timeout=2)
                end = time.monotonic() + 4
                while not (folder / 'guard.result').exists() and time.monotonic() < end:
                    time.sleep(.02)
                result = json.loads((folder / 'guard.result').read_text())
                expected = 'already-running' if running else ('restore-start-failed' if failed else 'restore-start-succeeded')
                self.assertEqual(result['result'], expected)
                self.assertEqual((folder / 'calls').read_text() if (folder / 'calls').exists() else '',
                                 '' if running else 'start\n')

    def test_command_deadline_and_output_cap(self):
        with self.assertRaises(M.Failure):
            M.command([sys.executable, '-c', 'import time;time.sleep(3)'], .05)
        with self.assertRaises(M.Failure):
            M.command([sys.executable, '-c', 'print("x"*10000)'], 2, 20)
        with self.assertRaises(M.Failure):
            M.command([sys.executable, '-c', 'import os,time;os.close(1);os.close(2);time.sleep(3)'], .05)

    def test_metrics_reject_duplicates_nonfinite_and_incomplete(self):
        parsed = M.parse_metrics('truenas_api_connected{target="memory-alpha"} 1\n# EOF\n')
        self.assertEqual(M.one(parsed, 'truenas_api_connected'), 1)
        parsed['truenas_api_connected'] *= 2
        with self.assertRaises(M.Failure):
            M.one(parsed, 'truenas_api_connected')
        for text in ['truenas_api_connected{target="memory-alpha"} NaN\n# EOF\n',
                     'truenas_api_connected{target="memory-alpha",target="x"} 1\n# EOF\n',
                     'truenas_api_connected{target="memory-alpha"} 1\n']:
            with self.subTest(text=text), self.assertRaises(M.Failure):
                M.parse_metrics(text)

    def test_fixed_length_body_may_close_socket_on_final_read(self):
        class Socket:
            closed = False
            def settimeout(self, _seconds):
                if self.closed:
                    raise OSError(9, 'Bad file descriptor')
        class Response:
            status, length = 200, 5
            def __init__(self, sock):
                self.sock = sock
            def read1(self, _limit):
                self.length = 0
                self.sock.closed = True
                return b'hello'
            def isclosed(self):
                return self.sock.closed
        class Connection:
            def __init__(self, *_args, **_kwargs):
                self.sock = Socket()
            def request(self, *_args, **_kwargs):
                pass
            def getresponse(self):
                return Response(self.sock)
            def close(self):
                pass
        with tempfile.TemporaryDirectory() as directory, patch.object(M.http.client, 'HTTPConnection', Connection):
            backend = M.LiveBackend({}, directory)
            self.assertEqual(backend.http('127.0.0.1', 9090, '/', 'direct', 10, 1), (200, 'hello'))

    def test_stopped_http_batch_fits_observation_tick(self):
        with tempfile.TemporaryDirectory() as directory:
            backend = M.LiveBackend({'clock_uncertainty_seconds': .1}, directory)
            clock = [0.0]
            backend.clock = lambda: clock[0]
            backend.raw = lambda _cid: {'Image': M.IMAGE, 'RestartCount': 0,
                'State': {'Running': False, 'OOMKilled': False}}
            backend.initial = {'restart_count': 0}
            backend.cid, backend.ip = 'a'*64, '127.0.0.1'
            backend.prom_at, backend.prom = 0, {'up': 0, 'sample': 0}
            deadlines = []
            def blocked(_host, _port, _path, _kind, _cap, seconds):
                deadlines.append(seconds)
                clock[0] += seconds
                return None, ''
            backend.http = blocked
            observation = backend.observe()
            self.assertLessEqual(sum(deadlines), 2.1)
            self.assertLessEqual(max(deadlines), 2)
            self.assertIsNone(observation['metrics_http'])

    def test_real_loopback_http_caps_deadline_and_connection_close(self):
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass
            def do_GET(self):
                self.send_response(200)
                self.send_header('Connection', 'close')
                if self.path == '/fixed':
                    self.send_header('Content-Length', '5')
                self.end_headers()
                try:
                    if self.path == '/slow':
                        for _ in range(10):
                            self.wfile.write(b'x')
                            self.wfile.flush()
                            time.sleep(.05)
                    else:
                        self.wfile.write(b'hello')
                except (BrokenPipeError, ConnectionResetError):
                    pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                backend = M.LiveBackend({}, directory)
                code, body = backend.http('127.0.0.1', server.server_port, '/', 'direct', 10, 1)
                self.assertEqual((code, body), (200, 'hello'))
                code, body = backend.http('127.0.0.1', server.server_port, '/fixed', 'direct', 10, 1)
                self.assertEqual((code, body), (200, 'hello'))
                with self.assertRaises(M.Failure):
                    backend.http('127.0.0.1', server.server_port, '/', 'direct', 2, 1)
                started = time.monotonic()
                try:
                    code, _ = backend.http('127.0.0.1', server.server_port, '/slow', 'direct', 100, .1)
                    self.assertIsNone(code)
                except M.Failure:
                    pass
                self.assertLess(time.monotonic() - started, .4)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


if __name__ == '__main__':
    unittest.main()
