#!/usr/bin/env python3
"""Profile synthetic fixtures and a production exporter in separate local cgroups."""
import argparse
import hashlib
import http.client
import importlib.util
import json
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import threading
import time
import uuid
from urllib.parse import urlsplit


class DockerStats(http.client.HTTPConnection):
    def __init__(self, endpoint):
        parsed = urlsplit(endpoint)
        if parsed.scheme != "unix" or not parsed.path:
            raise ValueError("profiling requires an explicitly selected local Unix Docker endpoint")
        super().__init__("localhost", timeout=5)
        self.endpoint = parsed.path

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self.endpoint)

    def sample(self, name):
        if not re.fullmatch(r"truenas-profile-[a-f0-9]+-(exporter|fixture)", name):
            raise ValueError("unexpected profile container name")
        try:
            self.request("GET", f"/containers/{name}/stats?stream=false&one-shot=true")
            response = self.getresponse()
            data = response.read(262145)
            if response.status != 200 or len(data) > 262144:
                raise RuntimeError("bounded Docker statistics request failed")
            value = json.loads(data)
            memory = value["memory_stats"]
            return {
                "charged_bytes": memory["usage"], "limit_bytes": memory["limit"],
                "kernel_peak_bytes": memory.get("max_usage"),
                "cpu_total_ns": value["cpu_stats"]["cpu_usage"]["total_usage"],
                "memory_stats": {k: v for k, v in memory.get("stats", {}).items()
                                 if isinstance(v, int)},
            }
        finally:
            self.close()


def control_path(root, name):
    relative = Path(name).relative_to("/control")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise ValueError("profile file must remain inside its synthetic control directory")
    return path


def bounded_json(path, limit=16384):
    with path.open("rb") as file:
        data = file.read(limit + 1)
    if len(data) > limit:
        raise ValueError("profile control JSON exceeds its bound")
    return json.loads(data)


def validate_launch(root, launch):
    env = launch["environment"]
    fixed = {
        "TRUENAS_POLL_INTERVAL_SECONDS": "1", "TRUENAS_API_TIMEOUT_SECONDS": "1",
        "TRUENAS_CACHE_TTL_SECONDS": "3", "TRUENAS_SYSTEM_ENABLED": "true",
        "TRUENAS_REALTIME_ENABLED": "true", "TRUENAS_REALTIME_TTL_SECONDS": "3",
        "RUST_LOG": "error",
    }
    if set(env) != set(fixed) | {"TRUENAS_TARGETS_FILE", "EXPORTER_LISTEN"}:
        raise ValueError("unexpected profile environment")
    if any(env[key] != value for key, value in fixed.items()):
        raise ValueError("profile settings differ from the reviewed synthetic workload")
    address, _, port = env["EXPORTER_LISTEN"].rpartition(":")
    if address != "127.0.0.1" or not port.isdigit() or not 0 < int(port) < 65536:
        raise ValueError("profile HTTP listener must be loopback")
    targets = bounded_json(control_path(root, env["TRUENAS_TARGETS_FILE"]))["targets"]
    if len(targets) != 16:
        raise ValueError("profile must contain sixteen synthetic targets")
    urls = set()
    for index, target in enumerate(targets):
        url = urlsplit(target["ws_url"])
        if (url.scheme != "wss" or url.hostname != "127.0.0.1" or not url.port
                or url.path != "/api/current" or url.username or url.password
                or url.query or url.fragment or target["name"] != f"t{index:02}"
                or target["username"] != "synthetic"):
            raise ValueError("profile target must be the generated loopback fixture")
        urls.add(target["ws_url"])
        with control_path(root, target["api_key_file"]).open("rb") as key:
            if key.read(64) != b"SYNTHETIC_OPERATIONAL_KEY":
                raise ValueError("profile requires its generated synthetic key")
        ca = control_path(root, target["ca_cert_file"])
        if not 0 < ca.stat().st_size <= 1048576:
            raise ValueError("invalid synthetic CA size")
    if len(urls) != 1:
        raise ValueError("profile targets must share the generated loopback server")
    return env


def atomic_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value) + "\n")
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context", required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--binaries", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    command = ["docker", "--context", args.context]
    def docker(*parts):
        return subprocess.run(command + list(parts), check=True, capture_output=True,
                              text=True, timeout=30).stdout.strip()
    # Reuse the existing failure-preserving, inventory-checked container cleanup.
    spec = importlib.util.spec_from_file_location("container_validation", Path(__file__).with_name("check-container.py"))
    validation = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(validation)
    endpoint = json.loads(docker("context", "inspect", args.context))[0]["Endpoints"]["docker"]["Host"]
    stats = DockerStats(endpoint)
    binaries = args.binaries.resolve()
    if not (binaries / "operational").is_file():
        raise ValueError("export the operational-test target first")
    image = json.loads(docker("image", "inspect", args.image))[0]
    if image["Config"]["User"] != "10001:10001":
        raise ValueError("profile requires the reviewed non-root runtime")
    token = uuid.uuid4().hex[:12]
    fixture = f"truenas-profile-{token}-fixture"
    exporter = f"truenas-profile-{token}-exporter"
    samples, errors = [], []
    phase = {"value": "startup"}
    lock = threading.Lock()
    report = {"evidence_kind": "synthetic_separate_cgroup_profile", "image_id": image["Id"],
              "harness_sha256": hashlib.sha256((binaries / "operational").read_bytes()).hexdigest()}
    log_process = None
    log_thread = None
    with tempfile.TemporaryDirectory(prefix="truenas-isolated-") as directory:
        root = Path(directory)
        root.chmod(0o777)  # Ephemeral synthetic control files are shared with UID 10001.
        with validation.cleanup_containers(command, [exporter, fixture]):
            try:
                common = ["--platform", "linux/amd64", "--read-only", "--tmpfs", "/tmp:rw,size=16m",
                          "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true"]
                docker("run", "-d", "--name", fixture, *common, "--network", "none",
                       "--memory", "256m", "--memory-swap", "256m", "--cpus", "1",
                       "--mount", f"type=bind,src={binaries},dst=/validation,readonly",
                       "--mount", f"type=bind,src={root},dst=/control",
                       "--env", "OPERATIONAL_CONTROL_DIR=/control", "--no-healthcheck",
                       "--entrypoint", "/validation/operational", args.image,
                       "isolated_cgroup_memory_profile", "--ignored", "--nocapture")
                log_process = subprocess.Popen(command + ["logs", "--follow", fixture],
                                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                log_lines = []
                def read_log():
                    for line in iter(lambda: log_process.stdout.readline(8193), ""):
                        if len(line) > 8192 or sum(map(len, log_lines)) + len(line) > 1048576:
                            errors.append("fixture log exceeded its bound")
                            break
                        log_lines.append(line)
                        print(line.rstrip(), flush=True)
                        match = re.search(r"resource_phase=([a-z0-9_]+)", line)
                        if match:
                            with lock:
                                phase["value"] = match[1]
                log_thread = threading.Thread(target=read_log, daemon=True)
                log_thread.start()
                deadline = time.monotonic() + 25
                while not (root / "launch.json").exists():
                    if time.monotonic() >= deadline:
                        raise RuntimeError("fixture did not publish its synthetic configuration")
                    time.sleep(0.1)
                env = validate_launch(root, bounded_json(root / "launch.json"))
                env_args = [part for key, value in env.items() for part in ("--env", f"{key}={value}")]
                docker("run", "-d", "--name", exporter, *common,
                       "--network", f"container:{fixture}", "--memory", "128m", "--memory-swap", "128m",
                       "--cpus", "0.5", "--mount", f"type=bind,src={root},dst=/control,readonly",
                       *env_args, args.image)
                (root / "started").write_text("started\n")
                started = time.monotonic()
                deadline = started + 930
                exit_state = None
                while time.monotonic() < deadline:
                    if errors:
                        raise RuntimeError(errors[0])
                    with lock:
                        label = phase["value"]
                    samples.append({"seconds": time.monotonic() - started, "phase": label,
                                    "exporter": stats.sample(exporter), "fixture": stats.sample(fixture)})
                    if (root / "stop").exists():
                        docker("stop", "--time", "5", exporter)
                        exit_state = json.loads(docker("inspect", exporter))[0]["State"]
                        atomic_json(root / "exit.json", {"code": exit_state["ExitCode"]})
                        break
                    time.sleep(1)
                if exit_state is None:
                    raise RuntimeError("isolated profile exceeded its deadline")
                fixture_exit = int(docker("wait", fixture))
                log_process.wait(timeout=5)
                log_thread.join(timeout=5)
                if errors or log_thread.is_alive():
                    raise RuntimeError("fixture log collection failed")
                if fixture_exit or exit_state["ExitCode"]:
                    raise RuntimeError("isolated fixture/exporter failed")
                report.update({"status": "passed", "exporter_exit_code": exit_state["ExitCode"],
                               "exporter_oom_killed": exit_state["OOMKilled"], "fixture_exit_code": fixture_exit,
                               "fixture_log": "".join(log_lines), "samples": samples})
            finally:
                if log_process is not None and log_process.poll() is None:
                    log_process.terminate()
                    log_process.wait(timeout=5)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    atomic_json(args.output, report)
    print(f"isolated_cgroup_profile=passed samples={len(samples)}", flush=True)


if __name__ == "__main__":
    main()
