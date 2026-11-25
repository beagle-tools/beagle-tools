#!/usr/bin/env bash
# The gate for one iPhone app (ios/) or Swift package (utils/swift). CI and
# `make check-ios` both run this script, so a green local run is a green CI
# run. macOS with Xcode only.
#
#   scripts/ci/ios.sh <dir>    e.g. ios (an app) or utils/swift (a package)
#
# The app dir holds exactly one <Name>.xcodeproj with a shared scheme of the
# same name. Tests run on an iPhone simulator: $IOS_SIMULATOR (a device name
# or UDID) when set, else the first iPhone of the newest installed iOS runtime.
set -euo pipefail

dir=${1:?usage: scripts/ci/ios.sh <app-dir>}
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/$dir"

# Two kinds of dir: an app (one <Name>.xcodeproj, shared scheme <Name>, test
# targets <Name>Tests and <Name>UITests) or a Swift package (Package.swift,
# tested through xcodebuild's <Name>-Package scheme on the same simulator).
package=""
if [ -f Package.swift ]; then
  package=$(sed -n 's/^ *name: "\([^"]*\)".*/\1/p' Package.swift | head -1)
  [ -n "$package" ] || { echo "FAIL: $dir/Package.swift: no package name" >&2; exit 1; }
  scheme="$package-Package"
else
  projects=(*.xcodeproj)
  if [ "${#projects[@]}" -ne 1 ] || [ ! -d "${projects[0]}" ]; then
    echo "FAIL: $dir must hold exactly one .xcodeproj or a Package.swift" >&2
    exit 1
  fi
  project=${projects[0]}
  scheme=${project%.xcodeproj}
fi

echo "==> [$dir] $(xcodebuild -version | head -1)"

# Lint what git tracks (or would track): never build output.
echo "==> [$dir] swift-format lint (strict, standards/swift/swift-format.json)"
swift_files=()
while IFS= read -r f; do swift_files+=("$f"); done < <(
  git ls-files --cached --others --exclude-standard -- '*.swift')
xcrun swift-format lint --strict --parallel \
  --configuration "$root/standards/swift/swift-format.json" "${swift_files[@]}"

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

# Dependencies come only from the committed Package.resolved (cargo --locked).
what=(-scheme "$scheme" -onlyUsePackageVersionsFromResolvedFile)
[ -n "$package" ] || what=(-project "$project" "${what[@]}")
# A package gets warnings-as-errors here, not in Package.swift: as an app's
# dependency Xcode builds it with -suppress-warnings, which conflicts.
# (Apps get it from standards/swift/Shared.xcconfig.)
[ -z "$package" ] || what+=(SWIFT_TREAT_WARNINGS_AS_ERRORS=YES GCC_TREAT_WARNINGS_AS_ERRORS=YES)
common=("${what[@]}" -destination "$destination" -derivedDataPath "$derived")

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
if [ -n "$package" ]; then
  echo "==> [$dir] package tests ($scheme, with coverage)"
  rm -rf "$derived/coverage.xcresult"
  xcodebuild test-without-building "${common[@]}" -parallel-testing-enabled NO \
    -enableCodeCoverage YES -resultBundlePath "$derived/coverage.xcresult" -quiet

  echo "==> [$dir] coverage floor (standards/swift/coverage.json)"
  xcrun xccov view --report --only-targets --json "$derived/coverage.xcresult" >"$derived/coverage.json"
  node "$root/scripts/ci/swift-coverage.mjs" "$dir" "$derived/coverage.json"

  # cargo doc -D warnings: a broken doc link or bad doc comment fails.
  echo "==> [$dir] docs (DocC, warnings as errors)"
  xcodebuild docbuild "${what[@]}" -destination "generic/platform=iOS Simulator" \
    -derivedDataPath "$derived" OTHER_DOCC_FLAGS="--warnings-as-errors" -quiet
else
  echo "==> [$dir] unit tests (${scheme}Tests)"
  xcodebuild test-without-building "${common[@]}" -only-testing:"${scheme}Tests" \
    -parallel-testing-enabled NO -quiet

  # UI tests drive a real app launch through the simulator's launch services,
  # which flake on CI runners independent of our code. A failing UI test gets
  # one retry; one that fails twice fails the gate. Unit tests never retry.
  echo "==> [$dir] UI tests (${scheme}UITests, one retry per failing test)"
  xcodebuild test-without-building "${common[@]}" -only-testing:"${scheme}UITests" \
    -parallel-testing-enabled NO -retry-tests-on-failure -test-iterations 2 -quiet
fi

# The static analyzer, warnings fail (it covers C/Objective-C; Swift's checks
# are the compiler's, already errors).
echo "==> [$dir] static analyzer"
analyze_log="$derived/analyze.log"
mkdir -p "$derived"
xcodebuild analyze "${what[@]}" -destination "generic/platform=iOS Simulator" \
  -derivedDataPath "$derived" CODE_SIGNING_ALLOWED=NO -quiet >"$analyze_log" 2>&1 || {
  cat "$analyze_log"
  exit 1
}
if grep -E "^/.*: warning: " "$analyze_log"; then
  echo "FAIL: [$dir] the static analyzer reported warnings" >&2
  exit 1
fi

echo "==> [$dir] release build (simulator, unsigned)"
xcodebuild build "${what[@]}" -configuration Release \
  -destination "generic/platform=iOS Simulator" -derivedDataPath "$derived" \
  CODE_SIGNING_ALLOWED=NO -quiet

echo "OK: $dir"
