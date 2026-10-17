# Homebrew cask for Tandem (Beagle Tandem). Release CI copies this file verbatim to
# beagle-tools/homebrew-tap as Casks/beagle-tandem.rb — edit it here, never in the tap.
#
# It always installs the newest release via the stable asset names on the
# rolling beagle-tandem-latest mirror, so it never needs a version bump; after
# install, Tandem updates itself in-app.
cask "beagle-tandem" do
  version :latest
  sha256 :no_check

  on_arm do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-macos-arm64.dmg"
  end
  on_intel do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-macos-x64.dmg"
  end

  name "Tandem"
  desc "Local-first code review that works with your agents"
  homepage "https://github.com/beagle-tools/beagle-tools/tree/main/tandem"

  app "Tandem.app"

  # Not notarized yet — clear quarantine so first launch works.
  postflight do
    system_command "/usr/bin/xattr",
                   args:  ["-dr", "com.apple.quarantine", "#{appdir}/Tandem.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.matthewmyrick.tandem",
    "~/Library/Caches/com.matthewmyrick.tandem",
    "~/Library/Preferences/com.matthewmyrick.tandem.plist",
    "~/Library/Saved Application State/com.matthewmyrick.tandem.savedState",
    "~/Library/WebKit/com.matthewmyrick.tandem",
  ]
end
