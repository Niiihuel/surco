"""The native interaction gate must reject empty, incomplete or skipped runs."""

from pathlib import Path
import runpy
import unittest


script = runpy.run_path(str(Path(__file__).with_name("run-ios-ui-tests.py")))


class NativeUIGateTests(unittest.TestCase):
    def test_all_native_interactions_must_execute_and_pass(self):
        complete = {"result": "Passed", "totalTestCount": 4, "passedTests": 4, "failedTests": 0, "skippedTests": 0}
        script["validate_summary"](complete)
        for incomplete in [
            {},
            {**complete, "totalTestCount": 0, "passedTests": 0},
            {**complete, "totalTestCount": 3, "passedTests": 3},
            {**complete, "failedTests": 1, "passedTests": 3, "result": "Failed"},
            {**complete, "skippedTests": 1, "passedTests": 3},
        ]:
            with self.subTest(summary=incomplete), self.assertRaises(RuntimeError):
                script["validate_summary"](incomplete)


if __name__ == "__main__":
    unittest.main()
