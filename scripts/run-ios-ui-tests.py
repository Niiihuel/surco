#!/usr/bin/env python3
"""Run native UI interactions on an explicitly supplied booted iPhone simulator.

Build Rust with build-ios.py first. The caller owns this simulator's lifecycle;
this script never boots, erases, shuts down or deletes a borrowed device.
"""

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess
import uuid


build = runpy.run_path(str(Path(__file__).with_name("build-ios.py")))
ROOT = build["ROOT"]
FIXTURE = ROOT / "ios/SurcoNativeUITests/SyntheticCollection.ron"
GEOMETRY = ROOT / "ios/SurcoNativeUITests/EntryGeometry.json"
SYMBOLS = ROOT / "ios/SurcoNativeUITests/SymbolNames.json"
BUNDLE_ID = "io.github.niiihuel.surco"


def validate_summary(summary):
    """A successful build or an empty test plan is not an interaction pass."""
    if summary.get("result") != "Passed" or summary.get("failedTests") != 0 or summary.get("skippedTests") != 0:
        raise RuntimeError("Native UI tests failed or skipped; inspect native-ui.xcresult")
    if summary.get("totalTestCount", 0) < 12 or summary.get("passedTests") != summary.get("totalTestCount"):
        raise RuntimeError("Expected all twelve native iOS tests to execute and pass")


@contextmanager
def record_simulator(udid, artifacts, env):
    """Finalize H.264 evidence before fixture restoration and simulator cleanup."""
    video = artifacts / "native-interactions.mp4"
    if video.exists():
        raise RuntimeError(f"Preserving existing recording: {video}")
    command = ["xcrun", "simctl", "io", udid, "recordVideo", "--codec=h264", str(video)]
    with (artifacts / "video-recording.log").open("w") as log:
        log.write(json.dumps(command) + "\n")
        log.flush()
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        body_failed = False
        try:
            yield
        except BaseException:
            body_failed = True
            raise
        finally:
            try:
                ended_early = process.poll() is not None
                if not ended_early:
                    try:
                        # simctl's documented Control-C stop path:
                        # https://developer.apple.com/videos/play/wwdc2020/10647/
                        process.send_signal(signal.SIGINT)
                    except ProcessLookupError:
                        pass
                try:
                    code = process.wait(timeout=30)
                except subprocess.TimeoutExpired as error:
                    process.kill()
                    process.wait(timeout=10)
                    raise RuntimeError("Simulator recording did not finalize after SIGINT; recorder killed") from error
                if ended_early or code != 0 or not video.is_file() or not video.stat().st_size:
                    raise RuntimeError(f"Simulator recording did not finalize successfully (exit {code}); inspect video-recording.log")
            except Exception as error:
                log.write(str(error) + "\n")
                if not body_failed:
                    raise


@contextmanager
def temporary_fixture(udid, artifacts, env, enabled):
    """Seed only Surco's state, preserving a borrowed simulator's original bytes."""
    if not enabled:
        yield
        return
    build["run"](["xcrun", "simctl", "terminate", udid, BUNDLE_ID], env,
        artifacts / "fixture-terminate-before.log", check=False, timeout=30)
    container = Path(build["run"](["xcrun", "simctl", "get_app_container", udid, BUNDLE_ID, "data"],
        env, artifacts / "fixture-container.log", quiet=True, timeout=60).strip())
    if not container.is_absolute() or not container.is_dir():
        raise RuntimeError("simctl did not return an existing absolute Surco application container")
    storage = container / "Library/Application Support/surco/app.ron"
    if storage.is_symlink():
        raise RuntimeError("Refusing to replace a symlink at Surco's fixture path")
    original = storage.read_bytes() if storage.exists() else None
    storage.parent.mkdir(parents=True, exist_ok=True)
    try:
        storage.write_bytes(FIXTURE.read_bytes())
        yield
    finally:
        # Stop the app before restoring so its normal persistence cannot overwrite
        # the original after this context exits. No other file is touched.
        try:
            build["run"](["xcrun", "simctl", "terminate", udid, BUNDLE_ID], env,
                artifacts / "fixture-terminate-after.log", check=False, timeout=30)
        finally:
            if original is None:
                storage.unlink(missing_ok=True)
            else:
                storage.write_bytes(original)


def run_tests(udid, config, derived, artifacts, env, *, seed_fixture=False):
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
        f"CURRENT_PROJECT_VERSION={env.get('GITHUB_RUN_NUMBER', '1')}",
        "-parallel-testing-enabled", "NO", "-maximum-concurrent-test-simulator-destinations", "1",
        "-resultBundlePath", str(result), "-only-testing:SurcoNativeUITests", "test",
    ]
    source = env.get("GITHUB_SHA") or build["run"](["git", "rev-parse", "HEAD"], env,
        artifacts / "source-sha.log", quiet=True, timeout=30).strip()
    (artifacts / "execution.json").write_text(json.dumps({"command": command, "simulator": udid,
        "source": source, "owns_simulator": False, "seed_fixture": seed_fixture,
        "video": "native-interactions.mp4", "video_codec": "h264",
        "fixture_sha256": hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
        "geometry_sha256": hashlib.sha256(GEOMETRY.read_bytes()).hexdigest(),
        "symbols_sha256": hashlib.sha256(SYMBOLS.read_bytes()).hexdigest()}, indent=2) + "\n")
    with temporary_fixture(udid, artifacts, env, seed_fixture):
        failure = None
        summary = None
        try:
            with record_simulator(udid, artifacts, env):
                build["run"](command, env, artifacts / "xcodebuild-native-ui.log", timeout=1500)
        except Exception as error:
            failure = error
        # Retain screenshots and the normal summary even when XCTest fails.
        # Try both exports; preserve the original failure if diagnostics also fail.
        if result.exists():
            evidence_commands = [
                (["xcrun", "xcresulttool", "get", "test-results", "summary", "--path", str(result)], "summary.json"),
                (["xcrun", "xcresulttool", "export", "attachments", "--path", str(result),
                    "--output-path", str(artifacts / "attachments")], "attachments-export.log"),
            ]
            for evidence, name in evidence_commands:
                try:
                    raw = build["run"](evidence, env, artifacts / name, timeout=120)
                    if name == "summary.json":
                        summary = json.loads(raw)
                except Exception as error:
                    if failure is None:
                        failure = error
        if failure is not None:
            raise failure
        validate_summary(summary or {})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--udid", required=True, help="Booted iPhone simulator owned by the caller")
    parser.add_argument("--xcconfig", type=Path, default=ROOT / "target/ios/simulator/Rust.xcconfig")
    parser.add_argument("--derived-data", type=Path, default=ROOT / "target/ios/simulator/DerivedData")
    parser.add_argument("--artifacts", type=Path, default=ROOT / "target/ios/simulator/artifacts/native-ui")
    parser.add_argument("--seed-fixture", action="store_true", help="Temporarily replace Surco app.ron with synthetic test data, then restore its original bytes")
    args = parser.parse_args()
    run_tests(args.udid, args.xcconfig.resolve(), args.derived_data.resolve(), args.artifacts.resolve(), os.environ.copy(), seed_fixture=args.seed_fixture)


if __name__ == "__main__":
    main()
