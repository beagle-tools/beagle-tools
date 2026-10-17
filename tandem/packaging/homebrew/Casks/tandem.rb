# Homebrew cask for Tandem. Lives in the matthewmyrick/homebrew-tap
# repo (Casks/tandem.rb); this copy is the source of truth.
#
# It always installs the latest release via the stable asset names, so
# it never needs a version bump — after install, Tandem updates itself
# in-app.
cask "tandem" do
  version :latest
  sha256 :no_check

  on_arm do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/tandem-latest/Tandem-macos-arm64.dmg"
  end
  on_intel do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/tandem-latest/Tandem-macos-x64.dmg"
  end

  name "Tandem"
  desc "Local-first code review that works with your agents"
  homepage "https://github.com/beagle-tools/beagle-tools/tree/main/tandem"

  app "Tandem.app"

  # Not notarized yet — clear quarantine so first launch works.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Tandem.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.matthewmyrick.tandem",
    "~/Library/Caches/com.matthewmyrick.tandem",
    "~/Library/Preferences/com.matthewmyrick.tandem.plist",
  ]
end
