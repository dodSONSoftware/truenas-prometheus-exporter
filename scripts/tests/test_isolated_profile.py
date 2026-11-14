import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "isolated_profile", Path(__file__).resolve().parents[1] / "profile-isolated-cgroups.py")
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


class IsolatedProfileTests(unittest.TestCase):
    def manifest(self, root):
        (root / "key").write_text("SYNTHETIC_OPERATIONAL_KEY")
        (root / "ca").write_text("synthetic-public-ca")
        targets = [{"name": f"t{i:02}", "ws_url": "wss://127.0.0.1:12345/api/current",
                    "username": "synthetic", "api_key_file": "/control/key",
                    "ca_cert_file": "/control/ca"} for i in range(16)]
        (root / "targets.json").write_text(json.dumps({"targets": targets}))
        launch = {"environment": {
            "TRUENAS_TARGETS_FILE": "/control/targets.json", "EXPORTER_LISTEN": "127.0.0.1:12346",
            "TRUENAS_POLL_INTERVAL_SECONDS": "1", "TRUENAS_API_TIMEOUT_SECONDS": "1",
            "TRUENAS_CACHE_TTL_SECONDS": "3", "TRUENAS_SYSTEM_ENABLED": "true",
            "TRUENAS_REALTIME_ENABLED": "true", "TRUENAS_REALTIME_TTL_SECONDS": "3", "RUST_LOG": "error"}}
        return launch, targets

    def test_generated_loopback_manifest_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            launch, _ = self.manifest(root)
            self.assertEqual(profile.validate_launch(root, launch), launch["environment"])

    def test_remote_endpoints_and_extra_environment_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            launch, targets = self.manifest(root)
            targets[0]["ws_url"] = "wss://example.invalid/api/current"
            (root / "targets.json").write_text(json.dumps({"targets": targets}))
            with self.assertRaises(ValueError):
                profile.validate_launch(root, launch)
            launch, _ = self.manifest(root)
            launch["environment"]["TRUENAS_API_KEY"] = "forbidden-inline-key"
            with self.assertRaises(ValueError):
                profile.validate_launch(root, launch)

    def test_non_synthetic_key_and_non_loopback_listener_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            launch, _ = self.manifest(root)
            (root / "key").write_text("not-the-synthetic-fixture")
            with self.assertRaises(ValueError):
                profile.validate_launch(root, launch)
            launch, _ = self.manifest(root)
            launch["environment"]["EXPORTER_LISTEN"] = "0.0.0.0:12346"
            with self.assertRaises(ValueError):
                profile.validate_launch(root, launch)

    def test_path_escape_and_remote_docker_context_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "escape").symlink_to("/etc/hosts")
            with self.assertRaises(ValueError):
                profile.control_path(root, "/control/escape")
            with self.assertRaises(ValueError):
                profile.DockerStats("tcp://example.invalid:2376")

    def test_control_json_is_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "large.json"
            path.write_text(" " * 16385)
            with self.assertRaises(ValueError):
                profile.bounded_json(path)
