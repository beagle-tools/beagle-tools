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
  [[ $IOS_SIMULATOR =~ ^[0-9A-F-]{36}$ ]] && destination="platform=iOS Simulator,id=$IOS_SIMULATOR"
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

echo "==> [$dir] build and test ($scheme: unit + UI tests)"
xcodebuild test \
  -project "$project" \
  -scheme "$scheme" \
  -destination "$destination" \
  -derivedDataPath "$derived" \
  -quiet

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
