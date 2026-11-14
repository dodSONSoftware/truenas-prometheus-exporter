import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "check_container", Path(__file__).resolve().parents[1] / "check-container.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def result(code=0, stdout=""):
    return subprocess.CompletedProcess([], code, stdout=stdout, stderr="")


class CleanupTests(unittest.TestCase):
    def test_success_follows_cleanup(self):
        events = []
        with patch.object(checker.subprocess, "run", side_effect=lambda *a, **k: events.append("removed") or result()):
            with checker.cleanup_containers(["docker"], ["owned"]):
                events.append("validated")
            events.append("passed")
        self.assertEqual(events, ["validated", "removed", "passed"])

    def test_removal_failure_attempts_remaining_containers(self):
        with patch.object(checker.subprocess, "run", side_effect=[
            result(1), result(stdout="container-id"), result()
        ]) as run:
            with self.assertRaisesRegex(RuntimeError, "owned-first"):
                with checker.cleanup_containers(["docker"], ["owned-first", "owned-second"]):
                    pass
        self.assertEqual(run.call_args_list[-1].args[0], ["docker", "rm", "--force", "owned-second"])

    def test_primary_error_survives_cleanup_failure(self):
        primary = ValueError("original validation failure")
        errors = io.StringIO()
        with patch.object(checker.subprocess, "run", side_effect=[result(1), result(1)]):
            with contextlib.redirect_stderr(errors):
                with self.assertRaises(ValueError) as caught:
                    with checker.cleanup_containers(["docker"], ["owned"]):
                        raise primary
        self.assertIs(caught.exception, primary)
        self.assertIn("Container cleanup failed: owned", errors.getvalue())

    def test_failed_start_with_no_container_needs_no_removal(self):
        with patch.object(checker.subprocess, "run", side_effect=[result(1), result()]):
            with checker.cleanup_containers(["docker"], ["owned"]):
                pass

    def test_failed_start_keeps_its_original_error(self):
        with patch.object(checker.subprocess, "run", side_effect=[result(1), result()]):
            with self.assertRaisesRegex(ValueError, "start failed"):
                with checker.cleanup_containers(["docker"], ["owned"]):
                    raise ValueError("start failed")

    def test_docker_execution_failure_attempts_all_cleanup(self):
        with patch.object(checker.subprocess, "run", side_effect=OSError("unavailable")) as run:
            with self.assertRaisesRegex(RuntimeError, "first.*second"):
                with checker.cleanup_containers(["docker"], ["first", "second"]):
                    pass
        self.assertEqual(run.call_count, 2)


if __name__ == "__main__":
    unittest.main()
