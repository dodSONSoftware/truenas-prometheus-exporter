#!/usr/bin/env python3
"""Validate a shell-free image against an explicitly selected local daemon."""
from contextlib import contextmanager
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid


EXPORTER = "/usr/local/bin/truenas-exporter"
PROBE = "/validation/runtime-probe"
KEY = "/validation/api-key"
CA = "/validation/ca.pem"
TARGETS = "/validation/targets.json"



@contextmanager
def cleanup_containers(command, containers):
    try:
        yield
    finally:
        validation_error = sys.exc_info()[1]
        failures = []
        for name in containers:
            try:
                removed = subprocess.run(command + ["rm", "--force", name],
                                         capture_output=True, text=True, check=False)
                if removed.returncode == 0:
                    continue
                # A failed start may never create a container. A successful list
                # distinguishes that case from an unavailable Docker daemon.
                remaining = subprocess.run(
                    command + ["container", "ls", "--all", "--filter",
                               "name=^/" + name + "$", "--format", "{{.ID}}"],
                    capture_output=True, text=True, check=False)
                if remaining.returncode == 0 and not remaining.stdout.strip():
                    continue
                failures.append(name + " (removal failed)")
            except OSError:
                failures.append(name + " (could not execute Docker cleanup)")
        if failures:
            message = "Container cleanup failed: " + ", ".join(failures)
            if validation_error is not None:
                print(message, file=sys.stderr)
            else:
                raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--docker", default="docker")
    parser.add_argument("--context", required=True)
    parser.add_argument("--image", default="local/truenas-rust-exporter:validation")
    parser.add_argument("--probe", type=Path, help="Previously exported validation-probe executable")
    args = parser.parse_args()
    command = [args.docker, "--context", args.context]
    root = Path(__file__).resolve().parent.parent

    def docker(*arguments):
        return subprocess.check_output(command + list(arguments), text=True).strip()

    endpoint = docker("context", "inspect", args.context, "--format", "{{.Endpoints.docker.Host}}")
    if not endpoint.startswith("unix://"):
        raise SystemExit("Refusing a non-local Docker endpoint; this check is for development only")
    image = json.loads(docker("image", "inspect", args.image))[0]
    if (image["Os"], image["Architecture"]) != ("linux", "amd64"):
        raise SystemExit("Expected a linux/amd64 validation image")

    with tempfile.TemporaryDirectory(prefix="truenas-container-check-") as temporary:
        directory = Path(temporary)
        probe = args.probe.resolve() if args.probe else directory / "probe" / "runtime-probe"
        if not args.probe:
            docker("build", "--platform", "linux/amd64", "--target", "validation-probe",
                   "--output", "type=local,dest=" + str(probe.parent),
                   "-f", str(root / "deployment/Dockerfile"), str(root))
        if not probe.is_file():
            raise SystemExit("Missing test-only runtime probe")
        key = directory / "api-key"
        ca = directory / "ca.pem"
        key.write_text("synthetic-container-validation-key")
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                        "-keyout", str(directory / "ca-key.pem"), "-out", str(ca),
                        "-days", "1", "-subj", "/CN=synthetic-fixture.invalid"],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        # Public synthetic test material only. Real secrets require operator-owned permissions.
        key.chmod(0o444)
        ca.chmod(0o444)
        targets = directory / "targets.json"
        targets.write_text(json.dumps({"targets": [
            {"name": name, "ws_url": f"wss://127.0.0.1:{port}/api/current",
             "username": "synthetic-user", "api_key_file": KEY, "ca_cert_file": CA}
            for name, port in [("alpha", 1), ("beta", 2)]
        ]}))
        targets.chmod(0o444)
        containers = []

        def start(environment, isolated=False):
            name = "truenas-validation-" + uuid.uuid4().hex[:12]
            containers.append(name)
            options = ["run", "--detach", "--name", name, "--platform", "linux/amd64",
                       "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true",
                       "--health-interval", "1s", "--health-start-period", "1s"]
            for source, target in [(probe, PROBE), (key, KEY), (ca, CA), (targets, TARGETS),
                                   (root / "LICENSE", "/validation/LICENSE")]:
                options += ["--mount", f"type=bind,src={source},dst={target},readonly"]
            for name_, value in environment.items():
                options += ["--env", name_ + "=" + value]
            options += ["--network", "none"] if isolated else ["--publish", "127.0.0.1::9110"]
            docker(*options, args.image)
            return name

        def healthy(name):
            deadline = time.monotonic() + 30
            while docker("inspect", "--format", "{{.State.Health.Status}}", name) != "healthy":
                if (time.monotonic() >= deadline or
                        docker("inspect", "--format", "{{.State.Running}}", name) != "true"):
                    raise RuntimeError("Container did not become healthy: " + name)
                time.sleep(0.25)
            info = json.loads(docker("inspect", name))[0]
            if info["Config"]["User"] != "10001:10001" or not info["HostConfig"]["ReadonlyRootfs"]:
                raise RuntimeError("Container user or filesystem policy is incorrect")
            docker("exec", name, EXPORTER, "healthcheck")

        def stop(name):
            docker("stop", "--time", "5", name)
            if docker("inspect", "--format", "{{.State.ExitCode}}", name) != "0":
                raise RuntimeError("Container did not stop cleanly on SIGTERM")

        with cleanup_containers(command, containers):
            mock = start({"EXPORTER_MOCK": "true"})
            healthy(mock)
            address = docker("port", mock, "9110/tcp")
            if not address.startswith("127.0.0.1:") or "\n" in address:
                raise RuntimeError("Expected one loopback-only published port")
            http = urllib.request.build_opener(urllib.request.ProxyHandler({}))

            def get(path):
                with http.open("http://" + address + path, timeout=3) as response:
                    if response.status != 200:
                        raise RuntimeError("Unexpected HTTP response")
                    return response.read().decode()

            get("/healthz")
            get("/readyz")
            metrics = get("/metrics")
            for expected in ('truenas_exporter_mock_mode 1', 'truenas_api_connected 0',
                             'truenas_pool_size_bytes{pool="example-pool"} 1000000'):
                if expected not in metrics:
                    raise RuntimeError("Missing synthetic metric: " + expected)
            docker("exec", mock, PROBE, KEY, CA)
            stop(mock)

            # No network access: validate real-mode file configuration and outage behavior.
            outage = start({"TRUENAS_WS_URL": "wss://127.0.0.1:1/api/current",
                            "TRUENAS_USERNAME": "synthetic-user", "TRUENAS_API_KEY_FILE": KEY,
                            "TRUENAS_CA_CERT_FILE": CA}, isolated=True)
            healthy(outage)
            docker("exec", outage, PROBE, KEY, CA, "--unready")
            stop(outage)

            multiple = start({"TRUENAS_TARGETS_FILE": TARGETS}, isolated=True)
            healthy(multiple)
            docker("exec", multiple, PROBE, KEY, CA, "--unready", "--targets")
            stop(multiple)
        print(json.dumps({"result": "passed", "image_id": image["Id"],
                          "platform": "linux/amd64", "checks": ["HTTP liveness/readiness/metrics",
                          "Docker healthcheck", "non-root UID/GID", "read-only filesystem",
                          "zero capability sets", "no-new-privileges", "license and CA bundle",
                          "no shell/package manager", "credential/private-CA mounts",
                          "network-isolated outage readiness", "multi-target configuration/metrics", "SIGTERM"]}, indent=2))


if __name__ == "__main__":
    main()
