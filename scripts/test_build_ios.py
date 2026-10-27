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
    def test_sideload_ipa_wraps_the_app_in_payload_and_keeps_it_executable(self):
        import zipfile
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory) / "Surco.app"
            (app / "Base.lproj").mkdir(parents=True)
            (app / "Surco").write_bytes(b"binary")
            (app / "Surco").chmod(0o755)
            (app / "Base.lproj" / "LaunchScreen.storyboardc").write_bytes(b"nib")
            ipa = Path(directory) / "Surco-unsigned.ipa"
            script["sideload_ipa"](app, ipa)
            with zipfile.ZipFile(ipa) as package:
                names = package.namelist()
                self.assertIn("Payload/Surco.app/Surco", names)
                self.assertIn("Payload/Surco.app/Base.lproj/LaunchScreen.storyboardc", names)
                self.assertTrue(all(name.startswith("Payload/Surco.app/") for name in names))
                self.assertEqual(package.getinfo("Payload/Surco.app/Surco").external_attr >> 16 & 0o111, 0o111)

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

    def test_boot_failure_retains_system_diagnostics_and_deletes_only_owned_simulator(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / "Surco.app"
            app.mkdir()
            (app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": "io.github.niiihuel.surco"}))
            artifacts = root / "artifacts"
            (artifacts / "logs").mkdir(parents=True)
            calls = []
            inventory = {"devices": {"com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                {"isAvailable": True, "name": "iPhone 17", "deviceTypeIdentifier": "phone"}]}}

            def fake_run(command, env, log, **options):
                calls.append((command, log.name))
                if "devices" in command:
                    return json.dumps(inventory)
                if "create" in command:
                    return "owned-simulator\n"
                if "bootstatus" in command:
                    raise RuntimeError("readiness failed")
                log.write_text("CoreLocationMigrator waiting")
                return ""

            globals_ = script["smoke_simulator"].__globals__
            with patch.dict(globals_, {"run": fake_run, "collect_crash_reports": lambda *args: []}), patch.object(time, "sleep"):
                with self.assertRaisesRegex(RuntimeError, "readiness failed"):
                    script["smoke_simulator"](app, "26.5", {}, artifacts)
            commands = [command for command, _ in calls]
            self.assertNotIn("launch", [part for command in commands for part in command])
            self.assertIn(["xcrun", "simctl", "delete", "owned-simulator"], commands)
            diagnostics = artifacts / "logs/simulator-startup-system.log"
            self.assertTrue(diagnostics.exists(), "a boot failure needs OS diagnostics before the owned device is deleted")
            self.assertIn("CoreLocationMigrator", diagnostics.read_text())
            diagnostic_index = next(index for index, (_, name) in enumerate(calls) if name == diagnostics.name)
            delete_index = commands.index(["xcrun", "simctl", "delete", "owned-simulator"])
            self.assertLess(diagnostic_index, delete_index)
            self.assertTrue(all(command[3] == "owned-simulator" for command in commands
                                if command[:3] in (["xcrun", "simctl", "terminate"], ["xcrun", "simctl", "shutdown"],
                                                   ["xcrun", "simctl", "delete"])))

    def test_early_boot_is_awaited_and_still_owned_by_the_smoke_cleanup(self):
        from concurrent.futures import Future
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / "Surco.app"
            app.mkdir()
            (app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": "io.github.niiihuel.surco"}))
            artifacts = root / "artifacts"
            (artifacts / "logs").mkdir(parents=True)
            calls = []

            def fake_run(command, env, log, **options):
                calls.append(command)
                if "bootstatus" in command:
                    raise RuntimeError("readiness failed")
                return ""

            booting = Future()
            booting.set_result("early-simulator")
            globals_ = script["smoke_simulator"].__globals__
            with patch.dict(globals_, {"run": fake_run, "collect_crash_reports": lambda *args: []}), patch.object(time, "sleep"):
                with self.assertRaisesRegex(RuntimeError, "readiness failed"):
                    script["smoke_simulator"](app, "26.5", {}, artifacts, booting)
            verbs = [command[2] for command in calls if command[:2] == ["xcrun", "simctl"]]
            self.assertNotIn("create", verbs, "the device booted during the build is reused")
            self.assertNotIn("boot", verbs)
            self.assertEqual(calls[0], ["xcrun", "simctl", "bootstatus", "early-simulator", "-b"])
            self.assertIn(["xcrun", "simctl", "delete", "early-simulator"], calls)

    def test_screenshot_must_complete_before_second_survival_check_and_smoke_record(self):
        for screenshot_times_out in (False, True):
            with self.subTest(screenshot_times_out=screenshot_times_out), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                app = root / "Surco.app"
                app.mkdir()
                bundle_id = "io.github.niiihuel.surco"
                (app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": bundle_id}))
                artifacts = root / "artifacts"
                (artifacts / "logs").mkdir(parents=True)
                calls = []
                inventory = {"devices": {"com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                    {"isAvailable": True, "name": "iPhone 17", "deviceTypeIdentifier": "phone"}]}}

                def fake_run(command, env, log, **options):
                    calls.append(command)
                    if "devices" in command:
                        return json.dumps(inventory)
                    if "create" in command:
                        return "owned-simulator\n"
                    if "launch" in command:
                        return f"{bundle_id}: 12345\n"
                    if "screenshot" in command:
                        # A file can exist even when simctl has not completed successfully.
                        Path(command[-1]).write_bytes(b"captured image")
                        if screenshot_times_out:
                            raise RuntimeError("screenshot timed out")
                    log.write_text("completed")
                    return ""

                globals_ = script["smoke_simulator"].__globals__
                with patch.dict(globals_, {"run": fake_run, "collect_crash_reports": lambda *args: []}), \
                        patch.object(time, "sleep"), patch.object(script["os"], "kill") as survival_check:
                    if screenshot_times_out:
                        with self.assertRaisesRegex(RuntimeError, "screenshot timed out"):
                            script["smoke_simulator"](app, "26.5", {}, artifacts)
                    else:
                        script["smoke_simulator"](app, "26.5", {}, artifacts)
                    self.assertEqual([call.args for call in survival_check.call_args_list],
                                     [(12345, 0)] * (1 if screenshot_times_out else 2))
                self.assertTrue((artifacts / "simulator.png").exists())
                self.assertEqual((artifacts / "smoke.json").exists(), not screenshot_times_out)
                self.assertEqual((artifacts / "logs/simulator-startup-system.log").exists(), screenshot_times_out)
                self.assertIn(["xcrun", "simctl", "delete", "owned-simulator"], calls)

    def test_native_interactions_use_the_owned_simulator_before_cleanup_and_propagate_failures(self):
        for interaction_fails in (False, True):
            with self.subTest(failure=interaction_fails), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                app = root / "Surco.app"
                app.mkdir()
                bundle_id = "io.github.niiihuel.surco"
                (app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": bundle_id}))
                artifacts = root / "artifacts"
                (artifacts / "logs").mkdir(parents=True)
                calls = []
                inventory = {"devices": {"com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                    {"isAvailable": True, "name": "iPhone 17", "deviceTypeIdentifier": "phone"}]}}

                def fake_run(command, env, log, **options):
                    calls.append(command)
                    log.write_text("completed")
                    if "devices" in command:
                        return json.dumps(inventory)
                    if "create" in command:
                        return "owned-simulator\n"
                    if "launch" in command:
                        return f"{bundle_id}: 12345\n"
                    if any(str(part).endswith("run-ios-ui-tests.py") for part in command) and interaction_fails:
                        raise RuntimeError("native interaction failed")
                    return ""

                globals_ = script["smoke_simulator"].__globals__
                with patch.dict(globals_, {"run": fake_run, "collect_crash_reports": lambda *args: []}), \
                        patch.object(time, "sleep"), patch.object(script["os"], "kill"):
                    if interaction_fails:
                        with self.assertRaisesRegex(RuntimeError, "native interaction failed"):
                            script["smoke_simulator"](app, "26.5", {"SURCO_IOS_NATIVE_UI_TESTS": "1"}, artifacts)
                    else:
                        script["smoke_simulator"](app, "26.5", {"SURCO_IOS_NATIVE_UI_TESTS": "1"}, artifacts)
                interactions = [command for command in calls if any(str(part).endswith("run-ios-ui-tests.py") for part in command)]
                self.assertEqual(len(interactions), 1, "native interactions are a required gate, without retries")
                command = interactions[0]
                self.assertEqual(command[command.index("--udid") + 1], "owned-simulator")
                self.assertIn("--seed-fixture", command)
                self.assertLess(calls.index(command), calls.index(["xcrun", "simctl", "delete", "owned-simulator"]))
                self.assertEqual(sum("create" in command for command in calls), 1, "reuse the startup-smoke simulator")
                self.assertEqual(sum("boot" in command for command in calls), 1)
                self.assertIn(["xcrun", "simctl", "shutdown", "owned-simulator"], calls)

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
