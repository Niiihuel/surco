"""The native interaction gate must reject empty, incomplete or skipped runs."""

from pathlib import Path
import json
import runpy
import tempfile
import unittest
from unittest.mock import patch


script = runpy.run_path(str(Path(__file__).with_name("run-ios-ui-tests.py")))


class NativeUIGateTests(unittest.TestCase):
    def test_fixture_does_not_access_the_simulator_without_explicit_opt_in(self):
        with patch.dict(script["build"], {"run": lambda *args, **kwargs: self.fail("fixture access requires opt-in")}):
            with script["temporary_fixture"]("unused", Path("unused"), {}, False):
                pass

    def test_all_native_interactions_must_execute_and_pass(self):
        complete = {"result": "Passed", "totalTestCount": 6, "passedTests": 6, "failedTests": 0, "skippedTests": 0}
        script["validate_summary"](complete)
        for incomplete in [
            {},
            {**complete, "totalTestCount": 0, "passedTests": 0},
            {**complete, "totalTestCount": 5, "passedTests": 5},
            {**complete, "failedTests": 1, "passedTests": 3, "result": "Failed"},
            {**complete, "skippedTests": 1, "passedTests": 3},
            {key: value for key, value in complete.items() if key != "failedTests"},
            {key: value for key, value in complete.items() if key != "skippedTests"},
        ]:
            with self.subTest(summary=incomplete), self.assertRaises(RuntimeError):
                script["validate_summary"](incomplete)

    def test_fixture_is_explicit_reversible_and_evidence_survives_xctest_failure(self):
        for fails in (False, True):
            for existing in (False, True):
                with self.subTest(fails=fails, existing=existing), tempfile.TemporaryDirectory() as directory:
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
                            return json.dumps({"result": "Passed", "totalTestCount": 6,
                                "passedTests": 6, "failedTests": 0, "skippedTests": 0})
                        return ""

                    with patch.dict(script["build"], {"run": fake_run}):
                        if fails:
                            with self.assertRaisesRegex(RuntimeError, "real XCTest failure"):
                                script["run_tests"]("00000000-0000-0000-0000-000000000001", config,
                                    root / "DerivedData", artifacts, {}, seed_fixture=True)
                        else:
                            script["run_tests"]("00000000-0000-0000-0000-000000000001", config,
                                root / "DerivedData", artifacts, {}, seed_fixture=True)
                    self.assertEqual(storage.read_bytes() if storage.exists() else None, original if existing else None)
                    self.assertEqual(other.read_bytes(), b"unchanged")
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
