"""Checks iOS launch resources, platform selection and linker ordering."""

from pathlib import Path
import json
import plistlib
import re
import runpy
import shlex
import subprocess
import tempfile
import time
import unittest
import xml.etree.ElementTree as ET
from unittest.mock import patch


script = runpy.run_path(str(Path(__file__).with_name("build-ios.py")))


class IOSBuildTests(unittest.TestCase):
    def test_launch_background_resolves_to_opaque_app_canvas_in_every_appearance(self):
        root = script["ROOT"]
        with (root / "ios/Info.plist").open("rb") as source:
            info = plistlib.load(source)
        self.assertNotIn("UILaunchScreen", info, "one system-owned launch screen")
        self.assertEqual(info.get("UIUserInterfaceStyle"), "Dark", "native surfaces match Surco's fixed dark appearance")
        storyboard = ET.parse(root / "ios" / f'{info["UILaunchStoryboardName"]}.storyboard').getroot()
        self.assertEqual(storyboard.get("launchScreen"), "YES")
        self.assertRegex(storyboard.get("toolsVersion", ""), r"^\d+(?:\.\d+)*$",
                         "ibtool needs the document toolsVersion while unarchiving")
        plugin = storyboard.find("dependencies/plugIn[@identifier='com.apple.InterfaceBuilder.IBCocoaTouchPlugin']")
        self.assertRegex(plugin.get("version", ""), r"^\d+(?:\.\d+)*$",
                         "keep the Interface Builder plugin version with the document metadata")
        view = storyboard.find(".//viewController/view")
        name = view.find("color[@key='backgroundColor']").get("name")
        self.assertIsNotNone(name, "avoid systemBackground changing to white in light mode")
        asset = json.loads((root / "ios/Assets.xcassets" / f"{name}.colorset/Contents.json").read_text())
        source = (root / "crates/app/src/view.rs").read_text()
        canvas = re.search(r"canvas:\s*Color::rgb\((\d+),\s*(\d+),\s*(\d+)\)", source)
        self.assertIsNotNone(canvas)
        expected = tuple(int(value) / 255 for value in canvas.groups())
        self.assertTrue(asset["colors"])
        for variant in asset["colors"]:
            self.assertEqual(variant["idiom"], "universal")
            color = variant["color"]
            self.assertEqual(color["color-space"], "srgb")
            rgba = color["components"]
            self.assertEqual(float(rgba["alpha"]), 1.0)
            for component, value in zip(("red", "green", "blue"), expected):
                self.assertAlmostEqual(float(rgba[component]), value, places=6)

    def test_launch_mark_uses_original_asset_and_fixed_centered_constraints(self):
        root = script["ROOT"]
        storyboard = ET.parse(root / "ios/LaunchScreen.storyboard").getroot()
        view = storyboard.find(".//viewController/view")
        mark = view.find(".//imageView")
        self.assertEqual(mark.get("contentMode"), "scaleAspectFit")
        for axis in ("width", "height"):
            constraint = mark.find(f"constraints/constraint[@firstAttribute='{axis}']")
            self.assertEqual(float(constraint.get("constant")), 96.0)
        for axis in ("centerX", "centerY"):
            constraint = view.find(f"constraints/constraint[@firstAttribute='{axis}']")
            self.assertEqual((constraint.get("firstItem"), constraint.get("secondItem"), constraint.get("secondAttribute")),
                             (mark.get("id"), view.get("id"), axis))
            self.assertEqual(float(constraint.get("constant", "0")), 0.0)
        asset = root / "ios/Assets.xcassets" / f'{mark.get("image")}.imageset'
        images = json.loads((asset / "Contents.json").read_text())["images"]
        self.assertTrue(images)
        original = (root / "ios/Assets.xcassets/AppIcon.appiconset/AppIcon.png").read_bytes()
        for image in images:
            self.assertEqual((asset / image["filename"]).read_bytes(), original, "reuse the original Surco mark without raster edits")
        project = (root / "ios/Surco.xcodeproj/project.pbxproj").read_text()
        resources = project.split("/* Begin PBXResourcesBuildPhase section */", 1)[1].split("/* End PBXResourcesBuildPhase section */", 1)[0]
        self.assertIn("LaunchScreen.storyboard in Resources", resources, "compile the launch storyboard into the app bundle")

    def test_crash_reports_preserve_only_current_surco_reports_before_simulator_deletion(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            host = home / "Library/Logs/DiagnosticReports"
            simulator = home / "Library/Developer/CoreSimulator/Devices/test-device/data/Library/Logs/CrashReporter"
            for source in (host, simulator):
                source.mkdir(parents=True)
                (source / "Surco-current.ips").write_text("native backtrace")
                (source / "OtherApp-current.ips").write_text("unrelated")
                stale = source / "Surco-stale.ips"
                stale.write_text("previous launch")
                stale.touch()
                script["os"].utime(stale, (0, 0))
            artifacts = home / "artifacts"
            artifacts.mkdir()
            reports = script["collect_crash_reports"]("test-device", artifacts, time.time(), home)
            self.assertEqual(set(reports), {"crashes/host/Surco-current.ips", "crashes/simulator-crash/Surco-current.ips"})
            self.assertTrue(all((artifacts / report).read_text() == "native backtrace" for report in reports))

    def test_cleanup_timeout_retains_logs_and_does_not_prevent_remaining_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "cleanup.log"
            with patch.object(subprocess, "run", side_effect=subprocess.TimeoutExpired("simctl", 30, output=b"partial log")):
                self.assertEqual(script["run"](["simctl"], {}, log, check=False), "")
                self.assertEqual(log.read_bytes(), b"partial log")
                with self.assertRaises(RuntimeError):
                    script["run"](["simctl"], {}, log)

    def test_linker_flags_keep_framework_order_duplicates_and_archive_spaces(self):
        archive = Path("/a checkout/target/libsurco_app.a")
        config = script["linker_configuration"](archive, "-framework UIKit -lc++ -framework Metal -lc++")
        self.assertEqual(shlex.split(config.split("=", 1)[1]),
                         ["$(inherited)", str(archive), "-framework", "UIKit", "-lc++", "-framework", "Metal", "-lc++"])
        with self.assertRaises(RuntimeError):
            script["linker_configuration"](archive, "")

    def test_simulator_uses_matching_sdk_and_an_available_iphone(self):
        inventory = {"devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-26-2": [
                {"isAvailable": True, "name": "iPhone newer", "deviceTypeIdentifier": "wrong-sdk"}],
            "com.apple.CoreSimulator.SimRuntime.iOS-18-5": [
                {"isAvailable": False, "name": "iPhone offline", "deviceTypeIdentifier": "unavailable"},
                {"isAvailable": True, "name": "iPad", "deviceTypeIdentifier": "tablet"},
                {"isAvailable": True, "name": "iPhone 16", "deviceTypeIdentifier": "phone"}],
        }}
        self.assertEqual(script["simulator_template"](inventory, "18.5.0"),
                         ("com.apple.CoreSimulator.SimRuntime.iOS-18-5", "phone"))
        with self.assertRaises(RuntimeError):
            script["simulator_template"](inventory, "18.4")


if __name__ == "__main__":
    unittest.main()
