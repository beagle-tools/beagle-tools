#!/usr/bin/env bash
# The gate for one iPhone app (an Xcode project in <tool>/ios/). CI and
# `make check-ios` both run this script, so a green local run is a green CI
# run. macOS with Xcode only.
#
#   scripts/ci/ios.sh <app-dir>    e.g. scout/ios
#
# The app dir holds exactly one <Name>.xcodeproj with a shared scheme of the
# same name. Tests run on an iPhone simulator: $IOS_SIMULATOR (a device name
# or UDID) when set, else the first iPhone of the newest installed iOS runtime.
set -euo pipefail

dir=${1:?usage: scripts/ci/ios.sh <app-dir>}
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/$dir"

projects=(*.xcodeproj)
if [ "${#projects[@]}" -ne 1 ] || [ ! -d "${projects[0]}" ]; then
  echo "FAIL: $dir must hold exactly one .xcodeproj" >&2
  exit 1
fi
project=${projects[0]}
scheme=${project%.xcodeproj}

echo "==> [$dir] $(xcodebuild -version | head -1)"

echo "==> [$dir] swift-format lint (strict, root .swift-format)"
xcrun swift-format lint --strict --recursive --parallel .

if [ -n "${IOS_SIMULATOR:-}" ]; then
  destination="platform=iOS Simulator,name=$IOS_SIMULATOR"
  if [[ $IOS_SIMULATOR =~ ^[0-9A-F-]{36}$ ]]; then
    udid=$IOS_SIMULATOR
    destination="platform=iOS Simulator,id=$udid"
  fi
else
  # The newest iOS runtime's first plain numbered iPhone ("iPhone 17"),
  # else its first iPhone of any kind.
  udid=$(xcrun simctl list devices available | awk '
    /^-- iOS / { version = $3; next }
    /^-- / { version = ""; next }
    version != "" && /iPhone/ {
      match($0, /\([0-9A-F-]{36}\)/); id = substr($0, RSTART + 1, RLENGTH - 2)
      plain = ($0 ~ /^ *iPhone [0-9]+ \(/) ? 0 : 1
      print version, plain, NR, id
    }' | sort -k1,1Vr -k2,2n -k3,3n | head -1 | cut -d' ' -f4)
  if [ -z "$udid" ]; then
    echo "FAIL: no iPhone simulator available (xcrun simctl list devices available)" >&2
    exit 1
  fi
  destination="platform=iOS Simulator,id=$udid"
fi
echo "==> [$dir] destination: $destination"

# Build output stays in the app dir (gitignored), not in ~/Library.
derived="$root/$dir/build"

common=(-project "$project" -scheme "$scheme" -destination "$destination" -derivedDataPath "$derived")

# Boot the simulator first and wait for it: a cold simulator on a CI runner
# can fail the first app launch ("Failed to send signal 19").
if [ -n "${udid:-}" ]; then
  echo "==> [$dir] booting the simulator"
  xcrun simctl boot "$udid" 2>/dev/null || true
  xcrun simctl bootstatus "$udid" -b >/dev/null
fi

echo "==> [$dir] build for testing"
xcodebuild build-for-testing "${common[@]}" -quiet

# One simulator, no clones: parallel clones are a frequent source of launch
# failures on CI and the suite is small.
echo "==> [$dir] unit tests (${scheme}Tests)"
xcodebuild test-without-building "${common[@]}" -only-testing:"${scheme}Tests" \
  -parallel-testing-enabled NO -quiet

# UI tests drive a real app launch through the simulator's launch services,
# which flake on CI runners independent of our code. A failing UI test gets
# one retry; one that fails twice fails the gate. Unit tests never retry.
echo "==> [$dir] UI tests (${scheme}UITests, one retry per failing test)"
xcodebuild test-without-building "${common[@]}" -only-testing:"${scheme}UITests" \
  -parallel-testing-enabled NO -retry-tests-on-failure -test-iterations 2 -quiet

echo "==> [$dir] release build (simulator, unsigned)"
xcodebuild build \
  -project "$project" \
  -scheme "$scheme" \
  -configuration Release \
  -destination "generic/platform=iOS Simulator" \
  -derivedDataPath "$derived" \
  CODE_SIGNING_ALLOWED=NO \
  -quiet

echo "OK: $dir"
