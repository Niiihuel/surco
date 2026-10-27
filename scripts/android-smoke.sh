#!/usr/bin/env bash
# Installs the debug APK on the connected emulator, starts Surco three scripted ways and keeps
# screenshots, logcat and the media session for review. Exits non-zero if a scenario crashed.
#
# Usage: scripts/android-smoke.sh path/to/app-debug.apk path/to/output-dir
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "Usage: $0 path/to/app-debug.apk path/to/output-dir" >&2
    exit 2
fi
apk="$1"
out="$2"
pkg=io.github.niiihuel.surco
root="$(cd "$(dirname "$0")/.." && pwd)"
fixture="$root/ios/SurcoNativeUITests/SyntheticCollection.ron"
failed=0
logcat_pid=""

mkdir -p "$out"
: > "$out/summary.txt"

stop_logcat() {
    if [ -n "$logcat_pid" ]; then
        kill "$logcat_pid" 2>/dev/null || true
        wait "$logcat_pid" 2>/dev/null || true
        logcat_pid=""
    fi
}
trap stop_logcat EXIT

adb wait-for-device
# The emulator's own launcher tends to stop answering under software rendering; its dialog
# would cover the screenshots, and it is not the app's.
adb shell settings put global hide_error_dialogs 1 || true
adb logcat -c
adb logcat -v threadtime > "$out/logcat.txt" 2>&1 &
logcat_pid=$!
adb install -r -g "$apk"

# Same fixture as iOS: it marks onboarding done, so the app starts on its home screen.
seed() {
    adb shell am force-stop "$pkg"
    adb push "$fixture" /data/local/tmp/app.ron >/dev/null
    adb shell "run-as $pkg sh -c 'mkdir -p files && cp /data/local/tmp/app.ron files/app.ron'"
}

# The remote side is a shell: the whole command is quoted, and the comma-separated script is
# single-quoted so that a value with spaces stays one argument for am.
launch() {
    if [ -n "$1" ]; then
        adb shell "am start -n $pkg/.MainActivity --esa surco.args '$1'" >/dev/null || true
    else
        adb shell "am start -n $pkg/.MainActivity" >/dev/null || true
    fi
}

shot() {
    adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1 || true
    adb exec-out screencap -p > "$out/$1" || true
}

# What the system says plays: the media session and the app's audio players.
playing() {
    { adb shell dumpsys media_session; echo; echo '--- audio players'; adb shell dumpsys audio | grep -iE -A2 'surco|AudioPlaybackConfiguration' ; } > "$out/$1" 2>&1 || true
}

record() {
    if [ -n "$(adb shell pidof "$pkg" | tr -d '\r')" ]; then
        echo "$1: ok" >> "$out/summary.txt"
    else
        echo "$1: crashed" >> "$out/summary.txt"
        failed=1
    fi
}

# Scenario 1: plain start.
seed
launch ""
sleep 25
shot 01-home.png
record home

# Scenario 2: open the Search tab after 4 seconds.
adb shell am force-stop "$pkg"
seed
launch "--click,mobile-search@4000"
sleep 12
shot 02-search.png
record search

# Scenario 3: search, play the first result, open the player, then background the app.
adb shell am force-stop "$pkg"
seed
# The song starts once the search has answered and its stream has come: the player is opened
# well after, from the mini-player that appears then.
launch "--search,The Marias Paranoia,--play,0,--click,mobile-playing@25000"
sleep 12
shot 03-results.png
sleep 23
shot 04-player.png
playing media-foreground.txt
adb shell input keyevent KEYCODE_HOME >/dev/null || true
sleep 10
shot 05-background.png
playing media-background.txt
record play

stop_logcat
grep -E 'Surco|AndroidRuntime|FATAL|DEBUG|libc|wgpu|panicked' "$out/logcat.txt" > "$out/logcat-surco.txt" || true

exit "$failed"
