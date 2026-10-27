#!/usr/bin/env python3
"""Build an unsigned iOS app; the simulator variant also checks launch and saves a screenshot."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import plistlib
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import zipfile
import time
import tomllib


ROOT = Path(__file__).resolve().parents[1]
PLATFORMS = {
    "simulator": ("aarch64-apple-ios-sim", "iphonesimulator", "iOS Simulator"),
    "device-unsigned": ("aarch64-apple-ios", "iphoneos", "iOS"),
}


def run(command, env, log, *, quiet=False, check=True, timeout=1800):
    print(f"+ {shlex.join(map(str, command))}", flush=True)
    try:
        result = subprocess.run(command, cwd=ROOT, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        log.write_bytes(error.stdout or b"")
        if check:
            raise RuntimeError(f"Command timed out; see {log}") from error
        print(f"Cleanup timed out; see {log}", flush=True)
        return ""
    log.write_text(result.stdout)
    if not quiet or result.returncode:
        print(result.stdout, end="", flush=True)
    if check and result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}); see {log}")
    return result.stdout


def linker_configuration(archive, native_flags):
    # rustc's order and repeated libraries can matter; never deduplicate these flags.
    flags = shlex.split(native_flags)
    if not flags:
        raise RuntimeError("rustc did not report native-static-libs")
    quote = lambda value: '"' + str(value).replace("\\", "\\\\").replace('"', '\\"') + '"'
    return "OTHER_LDFLAGS = $(inherited) " + " ".join(map(quote, [archive, *flags])) + "\n"


def simulator_template(inventory, sdk_version):
    def version(value):
        return (tuple(map(int, value.replace("-", ".").split("."))) + (0, 0, 0))[:3]

    for runtime, devices in inventory["devices"].items():
        if ".iOS-" not in runtime or version(runtime.split(".iOS-", 1)[1]) != version(sdk_version):
            continue
        for device in devices:
            if device.get("isAvailable") and device["name"].startswith("iPhone"):
                return runtime, device["deviceTypeIdentifier"]
    raise RuntimeError(f"No available iPhone simulator for SDK {sdk_version}; install its runtime in Xcode")


def collect_crash_reports(udid, artifacts, since, home=None):
    """Retain this launch's Apple reports before deleting the owned simulator."""
    home = Path.home() if home is None else Path(home)
    device_logs = home / "Library" / "Developer" / "CoreSimulator" / "Devices" / udid / "data" / "Library" / "Logs"
    sources = {
        "host": home / "Library" / "Logs" / "DiagnosticReports",
        "simulator-crash": device_logs / "CrashReporter",
        "simulator-diagnostic": device_logs / "DiagnosticReports",
    }
    reports = []
    for kind, directory in sources.items():
        if not directory.is_dir():
            continue
        for source in directory.rglob("Surco*"):
            if not source.is_file() or source.suffix not in (".ips", ".crash", ".diag"):
                continue
            if source.stat().st_mtime < since - 2:
                continue
            destination = artifacts / "crashes" / kind / source.relative_to(directory)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
            reports.append(str(destination.relative_to(artifacts)))
    (artifacts / "crash-reports.json").write_text(json.dumps(reports, indent=2) + "\n")
    return reports


def create_simulator(sdk_version, env, logs):
    inventory = json.loads(run(["xcrun", "simctl", "list", "devices", "available", "--json"],
                               env, logs / "simulators.json", quiet=True, timeout=60))
    runtime, device_type = simulator_template(inventory, sdk_version)
    return run(["xcrun", "simctl", "create", "Surco CI", device_type, runtime],
               env, logs / "simulator-create.log", timeout=60).strip()


def boot_early(sdk_version, env, logs):
    """A fresh simulator spends minutes migrating its data on first boot; started before the
    build, that happens while Rust and Xcode compile. A failed boot is retried by bootstatus -b."""
    udid = create_simulator(sdk_version, env, logs)
    run(["xcrun", "simctl", "boot", udid], env, logs / "simulator-boot.log", check=False, timeout=60)
    return udid


def smoke_simulator(app, sdk_version, env, artifacts, booting=None):
    logs = artifacts / "logs"
    with (app / "Info.plist").open("rb") as source:
        bundle_id = plistlib.load(source)["CFBundleIdentifier"]
    udid = booting.result() if booting else create_simulator(sdk_version, env, logs)
    launched_at = time.time()
    smoked = False
    try:
        if not booting:
            run(["xcrun", "simctl", "boot", udid], env, logs / "simulator-boot.log", timeout=60)
        # A fresh runtime performs OS data migration before any app can be installed.
        # This infrastructure budget is separate from Surco's launch/survival checks.
        run(["xcrun", "simctl", "bootstatus", udid, "-b"], env, logs / "simulator-ready.log", timeout=600)
        run(["xcrun", "simctl", "install", udid, str(app)], env, logs / "simulator-install.log", timeout=120)
        launch_env = {**env, "SIMCTL_CHILD_RUST_BACKTRACE": "1"}
        # Cold CI simulators can take over two minutes to finish LaunchServices registration.
        launched = run(["xcrun", "simctl", "launch", "--terminate-running-process",
                        f"--stdout={logs / 'app.stdout.log'}", f"--stderr={logs / 'app.stderr.log'}",
                        udid, bundle_id], launch_env, logs / "simulator-launch.log", timeout=300)
        match = re.search(re.escape(bundle_id) + r":\s*(\d+)", launched)
        if not match:
            raise RuntimeError("simctl did not return an application process ID")
        pid = int(match[1])
        time.sleep(8)
        try:
            os.kill(pid, 0)
        except ProcessLookupError as error:
            raise RuntimeError("Surco exited during simulator startup; inspect the app logs") from error
        # A cold CI display can write the PNG before simctl finishes its capture.
        # Require command success as well as the post-capture process check below.
        run(["xcrun", "simctl", "io", udid, "screenshot", str(artifacts / "simulator.png")],
            env, logs / "simulator-screenshot.log", timeout=180)
        os.kill(pid, 0)
        (artifacts / "smoke.json").write_text(json.dumps({"bundle_id": bundle_id, "pid": pid,
                                                         "launch_survived_seconds": 8}, indent=2) + "\n")
        if env.get("SURCO_IOS_NATIVE_UI_TESTS") == "1":
            # Reuse this ready device and Rust archive. The caller's finally block
            # retains diagnostics and owns cleanup even if a real interaction fails.
            run([sys.executable, str(ROOT / "scripts/run-ios-ui-tests.py"), "--udid", udid, "--seed-fixture"],
                env, logs / "native-ui-runner.log", timeout=2100)
        smoked = True
    finally:
        run(["xcrun", "simctl", "spawn", udid, "log", "show", "--last", "2m", "--style", "compact",
             "--predicate", f'process == "Surco" OR process == "ReportCrash" OR '
             f'eventMessage CONTAINS "{bundle_id}"'], env, logs / "simulator-system.log",
            quiet=True, check=False, timeout=60)
        if not smoked:
            # Keep the OS services excluded by the app-only log, before deleting this device.
            run(["xcrun", "simctl", "spawn", udid, "log", "show", "--last", "10m", "--style", "compact",
                 "--predicate", 'process == "locationd" OR process == "installd" OR process == "SpringBoard" OR '
                 'process CONTAINS[c] "Migrator" OR subsystem CONTAINS[c] "CoreLocation" OR '
                 'eventMessage CONTAINS[c] "migration"'], env, logs / "simulator-startup-system.log",
                quiet=True, check=False, timeout=60)
            run(["log", "show", "--last", "10m", "--style", "compact", "--predicate",
                 'process == "CoreSimulatorService" OR subsystem BEGINSWITH "com.apple.CoreSimulator"'],
                env, logs / "coresimulator-host.log", quiet=True, check=False, timeout=60)
        # CrashReporter may finish writing the report after the process has already exited.
        for attempt in range(6):
            try:
                reports = collect_crash_reports(udid, artifacts, launched_at)
            except OSError as error:
                (logs / "crash-collection.log").write_text(str(error) + "\n")
                break
            if reports or attempt == 5:
                break
            time.sleep(1)
        run(["xcrun", "simctl", "terminate", udid, bundle_id], env, logs / "simulator-terminate.log",
            quiet=True, check=False, timeout=30)
        run(["xcrun", "simctl", "shutdown", udid], env, logs / "simulator-shutdown.log",
            quiet=True, check=False, timeout=30)
        run(["xcrun", "simctl", "delete", udid], env, logs / "simulator-delete.log",
            quiet=True, check=False, timeout=30)


def sideload_ipa(app, path):
    """An unsigned IPA: a sideloading tool re-signs it with the installer's own Apple ID."""
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as package:
        for file in sorted(app.rglob("*")):
            # ZipFile.write keeps each file's mode, so the executable stays runnable.
            package.write(file, Path("Payload") / app.name / file.relative_to(app))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=PLATFORMS)
    args = parser.parse_args()
    target, sdk, destination = PLATFORMS[args.platform]
    output = ROOT / "target" / "ios" / args.platform
    artifacts = output / "artifacts"
    logs = artifacts / "logs"
    if artifacts.exists():
        shutil.rmtree(artifacts)
    logs.mkdir(parents=True, exist_ok=True)
    (artifacts / "ABOUT.txt").write_text(
        "Surco for ARM64 iOS Simulator; cannot be installed on an iPhone.\n"
        if args.platform == "simulator" else
        "UNSIGNED iPhone build. Requires Apple signing and provisioning before device installation.\n")
    if sys.platform != "darwin":
        raise RuntimeError("Building iOS requires macOS with Xcode; use the iOS GitHub Actions workflow")

    env = os.environ.copy()
    env["CARGO_TERM_COLOR"] = "never"
    env.setdefault("IPHONEOS_DEPLOYMENT_TARGET", "15.0")
    xcode = run(["xcodebuild", "-version"], env, logs / "xcode-version.log", timeout=60)
    rust = run(["rustc", "-vV"], env, logs / "rust-version.log", timeout=60)
    env["SDKROOT"] = run(["xcrun", "--sdk", sdk, "--show-sdk-path"], env,
                         logs / "sdk-path.log", timeout=60).strip()
    sdk_version = run(["xcrun", "--sdk", sdk, "--show-sdk-version"], env,
                      logs / "sdk-version.log", timeout=60).strip()
    booting = ThreadPoolExecutor(max_workers=1).submit(boot_early, sdk_version, env, logs) \
        if args.platform == "simulator" else None
    with (ROOT / "Cargo.toml").open("rb") as source:
        version = tomllib.load(source)["workspace"]["package"]["version"]
    (artifacts / "build.json").write_text(json.dumps({"target": target, "sdk": sdk_version,
        "xcode": xcode.strip(), "rust": rust.strip(), "version": version,
        "deployment_target": env["IPHONEOS_DEPLOYMENT_TARGET"],
        "commit": env.get("GITHUB_SHA"), "signed": False}, indent=2) + "\n")

    # Keep rustc's output beside DerivedData so a cached Cargo build can reuse it.
    native_flags = output / "native-static-libs.txt"
    run(["cargo", "rustc", "--locked", "--release", "-p", "surco", "--lib", "--target", target,
         "--target-dir", str(ROOT / "target"), "--", "--print", f"native-static-libs={native_flags}"],
        env, logs / "cargo.log", timeout=3600)
    archive = ROOT / "target" / target / "release" / "libsurco_app.a"
    if not archive.is_file() or not native_flags.is_file():
        raise RuntimeError("Missing Rust archive/native linker flags; clean package surco and retry")
    shutil.copyfile(native_flags, artifacts / native_flags.name)
    config = output / "Rust.xcconfig"
    config.write_text(linker_configuration(archive, native_flags.read_text()))
    derived = output / "DerivedData"
    run(["xcodebuild", "-project", str(ROOT / "ios" / "Surco.xcodeproj"), "-scheme", "Surco",
         "-configuration", "Release", "-sdk", sdk, "-destination", f"generic/platform={destination}",
         "-derivedDataPath", str(derived), "-xcconfig", str(config), "CODE_SIGNING_ALLOWED=NO",
         f"IPHONEOS_DEPLOYMENT_TARGET={env['IPHONEOS_DEPLOYMENT_TARGET']}", f"MARKETING_VERSION={version}",
         f"CURRENT_PROJECT_VERSION={env.get('GITHUB_RUN_NUMBER', '1')}", "build"],
        env, logs / "xcodebuild.log")
    app = derived / "Build" / "Products" / f"Release-{sdk}" / "Surco.app"
    with (app / "Info.plist").open("rb") as source:
        executable = app / plistlib.load(source)["CFBundleExecutable"]
    if not os.access(executable, os.X_OK):
        raise RuntimeError(f"Missing executable in {app}")
    run(["xcrun", "vtool", "-show-build", str(executable)], env, logs / "binary-platform.log", timeout=60)
    # Tar preserves executable modes and symlinks when GitHub stores the artifact.
    with tarfile.open(artifacts / f"surco-ios-{args.platform}-arm64.tar.gz", "w:gz") as package:
        package.add(app, arcname="Surco.app")
    if args.platform == "device-unsigned":
        sideload_ipa(app, artifacts / "Surco-unsigned.ipa")
    if args.platform == "simulator":
        smoke_simulator(app, sdk_version, env, artifacts, booting)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError) as error:
        print(f"iOS build failed: {error}", file=sys.stderr)
        sys.exit(1)
