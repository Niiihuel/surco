"""The native interaction gate must reject empty, incomplete or skipped runs."""

from pathlib import Path
import json
import runpy
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch


script = runpy.run_path(str(Path(__file__).with_name("run-ios-ui-tests.py")))


class NativeUIGateTests(unittest.TestCase):
    def test_video_finalizes_on_success_and_test_failure(self):
        for fails in (False, True):
            with self.subTest(fails=fails), tempfile.TemporaryDirectory() as directory:
                artifacts = Path(directory)
                process = Mock()
                process.poll.return_value = None
                def finalized(timeout):
                    (artifacts / "native-interactions.mp4").write_bytes(b"finalized recording")
                    return 0
                process.wait.side_effect = finalized
                with patch.object(subprocess, "Popen", return_value=process) as popen:
                    if fails:
                        with self.assertRaisesRegex(RuntimeError, "XCTest failed"):
                            with script["record_simulator"]("borrowed-udid", artifacts, {}):
                                raise RuntimeError("XCTest failed")
                    else:
                        with script["record_simulator"]("borrowed-udid", artifacts, {}):
                            pass
                self.assertEqual(popen.call_args.args[0], ["xcrun", "simctl", "io", "borrowed-udid",
                    "recordVideo", "--codec=h264", str(artifacts / "native-interactions.mp4")])
                process.send_signal.assert_called_once_with(signal.SIGINT)
                process.wait.assert_called_once_with(timeout=30)
                process.kill.assert_not_called()
                self.assertTrue((artifacts / "native-interactions.mp4").is_file())

    def test_stuck_video_is_killed_and_reaped_with_a_bounded_wait(self):
        with tempfile.TemporaryDirectory() as directory:
            process = Mock()
            process.poll.return_value = None
            process.wait.side_effect = [subprocess.TimeoutExpired("recordVideo", 30), 0]
            with patch.object(subprocess, "Popen", return_value=process):
                with self.assertRaisesRegex(RuntimeError, "recording.*finalize"):
                    with script["record_simulator"]("borrowed-udid", Path(directory), {}):
                        pass
            process.send_signal.assert_called_once_with(signal.SIGINT)
            process.kill.assert_called_once_with()
            self.assertEqual(process.wait.call_args_list, [unittest.mock.call(timeout=30), unittest.mock.call(timeout=10)])

    def test_an_early_recorder_exit_cannot_pass_as_motion_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            process = Mock()
            process.poll.return_value = 7
            process.wait.return_value = 7
            with patch.object(subprocess, "Popen", return_value=process):
                with self.assertRaisesRegex(RuntimeError, "recording.*finalize"):
                    with script["record_simulator"]("borrowed-udid", Path(directory), {}):
                        pass
            process.send_signal.assert_not_called()
            process.kill.assert_not_called()
            process.wait.assert_called_once_with(timeout=30)

    def test_fixture_does_not_access_the_simulator_without_explicit_opt_in(self):
        with patch.dict(script["build"], {"run": lambda *args, **kwargs: self.fail("fixture access requires opt-in")}):
            with script["temporary_fixture"]("unused", Path("unused"), {}, False):
                pass

    def test_all_native_interactions_must_execute_and_pass(self):
        complete = {"result": "Passed", "totalTestCount": 12, "passedTests": 12, "failedTests": 0, "skippedTests": 0}
        script["validate_summary"](complete)
        for incomplete in [
            {},
            {**complete, "totalTestCount": 0, "passedTests": 0},
            {**complete, "totalTestCount": 5, "passedTests": 5},
            {**complete, "totalTestCount": 6, "passedTests": 6},
            {**complete, "totalTestCount": 8, "passedTests": 8},
            {**complete, "totalTestCount": 9, "passedTests": 9},
            {**complete, "totalTestCount": 10, "passedTests": 10},
            {**complete, "passedTests": 8},
            {**complete, "passedTests": 10},
            {**complete, "result": "Failed"},
            {**complete, "failedTests": 1},
            {**complete, "skippedTests": 1},
            {**complete, "failedTests": 1, "passedTests": 3, "result": "Failed"},
            {**complete, "skippedTests": 1, "passedTests": 3},
            {key: value for key, value in complete.items() if key != "failedTests"},
            {key: value for key, value in complete.items() if key != "skippedTests"},
        ]:
            with self.subTest(summary=incomplete), self.assertRaises(RuntimeError):
                script["validate_summary"](incomplete)

    def test_fixture_is_explicit_reversible_and_evidence_survives_xctest_failure(self):
        for fails, video_stuck in [(False, False), (True, False), (False, True), (True, True)]:
            for existing in (False, True):
                with self.subTest(fails=fails, video_stuck=video_stuck, existing=existing), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    config = root / "Rust.xcconfig"
                    config.write_text("OTHER_LDFLAGS = archive\n")
                    container = root / "ApplicationContainer"
                    storage = container / "Library/Application Support/surco/app.ron"
                    storage.parent.mkdir(parents=True)
                    original = b"original app state\x00\xff"
                    if existing:
                        storage.write_bytes(original)
                    other = storage.with_name("keep-download")
                    other.write_bytes(b"unchanged")
                    artifacts = root / "artifacts"
                    calls = []
                    process = Mock()
                    process.poll.return_value = None
                    def finalized(timeout):
                        if video_stuck and process.wait.call_count == 1:
                            raise subprocess.TimeoutExpired("recordVideo", 30)
                        (artifacts / "native-interactions.mp4").write_bytes(b"finalized recording")
                        return 0
                    process.wait.side_effect = finalized

                    def fake_run(command, env, log, **kwargs):
                        calls.append(command)
                        log.write_text("command log\n")
                        if "get_app_container" in command:
                            self.assertEqual(command[-2:], ["io.github.niiihuel.surco", "data"])
                            return str(container) + "\n"
                        if command[0] == "git":
                            return "fixture-source-sha\n"
                        if command[0] == "xcodebuild":
                            self.assertEqual(storage.read_bytes(), script["FIXTURE"].read_bytes())
                            (artifacts / "native-ui.xcresult").mkdir()
                            # XCTest changes the app's own state; restoration must still happen.
                            storage.write_bytes(b"modified by XCTest")
                            if fails:
                                raise RuntimeError("real XCTest failure")
                        if command[:4] == ["xcrun", "xcresulttool", "get", "test-results"]:
                            return json.dumps({"result": "Passed", "totalTestCount": 12,
                                "passedTests": 12, "failedTests": 0, "skippedTests": 0})
                        return ""

                    with patch.dict(script["build"], {"run": fake_run}), patch.object(subprocess, "Popen", return_value=process):
                        if fails or video_stuck:
                            with self.assertRaisesRegex(RuntimeError, "real XCTest failure" if fails else "recording.*finalize"):
                                script["run_tests"]("00000000-0000-0000-0000-000000000001", config,
                                    root / "DerivedData", artifacts, {}, seed_fixture=True)
                        else:
                            script["run_tests"]("00000000-0000-0000-0000-000000000001", config,
                                root / "DerivedData", artifacts, {}, seed_fixture=True)
                    self.assertEqual(storage.read_bytes() if storage.exists() else None, original if existing else None)
                    self.assertEqual(other.read_bytes(), b"unchanged")
                    process.send_signal.assert_called_once_with(signal.SIGINT)
                    if video_stuck:
                        process.kill.assert_called_once_with()
                        self.assertEqual(process.wait.call_args_list, [unittest.mock.call(timeout=30), unittest.mock.call(timeout=10)])
                    else:
                        process.kill.assert_not_called()
                        process.wait.assert_called_once_with(timeout=30)
                    self.assertTrue((artifacts / "native-interactions.mp4").is_file())
                    self.assertTrue(any(command[:4] == ["xcrun", "xcresulttool", "export", "attachments"] for command in calls))
                    self.assertTrue(any(command[:4] == ["xcrun", "xcresulttool", "get", "test-results"] for command in calls))
                    self.assertFalse(any(action in command for command in calls for action in ("boot", "erase", "delete", "shutdown")))
                    metadata = json.loads((artifacts / "execution.json").read_text())
                    self.assertEqual(metadata["source"], "fixture-source-sha")
                    self.assertEqual(len(metadata["fixture_sha256"]), 64)
                    self.assertEqual(len(metadata["geometry_sha256"]), 64)
                    self.assertEqual(len(metadata["symbols_sha256"]), 64)


if __name__ == "__main__":
    unittest.main()
