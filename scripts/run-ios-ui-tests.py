#!/usr/bin/env python3
"""Run native UI interactions on an explicitly supplied booted iPhone simulator.

Build Rust with build-ios.py first. The caller owns this simulator's lifecycle;
this script never boots, erases, shuts down or deletes a borrowed device.
"""

import argparse
import json
import os
from pathlib import Path
import runpy
import uuid


build = runpy.run_path(str(Path(__file__).with_name("build-ios.py")))
ROOT = build["ROOT"]


def validate_summary(summary):
    """A successful build or an empty test plan is not an interaction pass."""
    if summary.get("result") != "Passed" or summary.get("failedTests", 0) or summary.get("skippedTests", 0):
        raise RuntimeError("Native UI tests failed or skipped; inspect native-ui.xcresult")
    if summary.get("totalTestCount", 0) < 4 or summary.get("passedTests") != summary.get("totalTestCount"):
        raise RuntimeError("Expected all four native UI interactions to execute and pass")


def run_tests(udid, config, derived, artifacts, env):
    uuid.UUID(udid)
    if not config.is_file():
        raise RuntimeError(f"Build the simulator Rust archive first; missing {config}")
    artifacts.mkdir(parents=True, exist_ok=True)
    result = artifacts / "native-ui.xcresult"
    if result.exists():
        raise RuntimeError(f"Preserving existing test evidence; choose a new artifacts path: {result}")
    command = [
        "xcodebuild", "-project", str(ROOT / "ios/Surco.xcodeproj"), "-scheme", "SurcoNativeUI",
        "-configuration", "Release", "-sdk", "iphonesimulator", "-destination", f"id={udid}",
        "-derivedDataPath", str(derived), "-xcconfig", str(config), "CODE_SIGNING_ALLOWED=NO",
        "-parallel-testing-enabled", "NO", "-maximum-concurrent-test-simulator-destinations", "1",
        "-resultBundlePath", str(result), "-only-testing:SurcoNativeUITests", "test",
    ]
    (artifacts / "execution.json").write_text(json.dumps({"command": command, "simulator": udid,
        "source": env.get("GITHUB_SHA"), "owns_simulator": False}, indent=2) + "\n")
    build["run"](command, env, artifacts / "xcodebuild-native-ui.log", timeout=1500)
    raw = build["run"](["xcrun", "xcresulttool", "get", "test-results", "summary", "--path", str(result)],
        env, artifacts / "summary.json", timeout=60)
    validate_summary(json.loads(raw))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--udid", required=True, help="Booted iPhone simulator owned by the caller")
    parser.add_argument("--xcconfig", type=Path, default=ROOT / "target/ios/simulator/Rust.xcconfig")
    parser.add_argument("--derived-data", type=Path, default=ROOT / "target/ios/simulator/DerivedData")
    parser.add_argument("--artifacts", type=Path, default=ROOT / "target/ios/simulator/artifacts/native-ui")
    args = parser.parse_args()
    run_tests(args.udid, args.xcconfig.resolve(), args.derived_data.resolve(), args.artifacts.resolve(), os.environ.copy())


if __name__ == "__main__":
    main()
