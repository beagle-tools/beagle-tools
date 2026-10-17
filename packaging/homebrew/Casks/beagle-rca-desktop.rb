# Homebrew cask for Beagle (Beagle RCA). Release CI copies this file verbatim to
# beagle-tools/homebrew-tap as Casks/beagle-rca-desktop.rb — edit it here, never in the tap.
#
# It always installs the newest release via the stable asset names on the
# rolling beagle-rca-latest mirror, so it never needs a version bump; after
# install, Beagle updates itself in-app.
cask "beagle-rca-desktop" do
  version :latest
  sha256 :no_check

  on_arm do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-rca-latest/Beagle-macos-arm64.dmg"
  end
  on_intel do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-rca-latest/Beagle-macos-x64.dmg"
  end

  name "Beagle"
  desc "Desktop viewer for Beagle root-cause analysis workspaces"
  homepage "https://github.com/beagle-tools/beagle-tools/tree/main/rca/desktop"

  app "Beagle.app"

  # Not notarized yet — clear quarantine so first launch works.
  postflight do
    system_command "/usr/bin/xattr",
                   args:  ["-dr", "com.apple.quarantine", "#{appdir}/Beagle.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.matthewmyrick.beagle",
    "~/Library/Caches/com.matthewmyrick.beagle",
    "~/Library/Preferences/com.matthewmyrick.beagle.plist",
    "~/Library/Saved Application State/com.matthewmyrick.beagle.savedState",
    "~/Library/WebKit/com.matthewmyrick.beagle",
  ]
end
