"""The iOS preference bridge keeps a matching C/Rust ABI without desktop UIKit linkage."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]


class IOSAccessibilityTests(unittest.TestCase):
    def test_reduce_motion_binding_matches_the_public_uikit_export(self):
        rust = (ROOT / "crates/app/src/ios_ui.rs").read_text()
        binding = re.search(r"fn (surco_reduce_motion_enabled)\(\) -> i32;", rust)
        self.assertIsNotNone(binding, "Rust declares the integer C bridge ABI")
        native = (ROOT / "ios/SurcoView.m").read_text()
        export = re.search(rf"int {binding.group(1)}\(void\)\s*\{{(.*?)\n\}}", native, re.S)
        self.assertIsNotNone(export, "UIKit exports the symbol Rust calls")
        self.assertIn("UIAccessibilityIsReduceMotionEnabled()", export.group(1))
        self.assertRegex(rust, r"pub fn reduce_motion_enabled\(\) -> bool")


if __name__ == "__main__":
    unittest.main()
