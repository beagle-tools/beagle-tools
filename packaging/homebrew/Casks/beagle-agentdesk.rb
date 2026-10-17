# Homebrew cask for Agentdesk (Beagle Workflows). Release CI copies this file verbatim to
# beagle-tools/homebrew-tap as Casks/beagle-agentdesk.rb — edit it here, never in the tap.
#
# It always installs the newest release via the stable asset names on the
# rolling beagle-workflows-latest mirror, so it never needs a version bump; after
# install, Agentdesk updates itself in-app.
cask "beagle-agentdesk" do
  version :latest
  sha256 :no_check

  on_arm do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-workflows-latest/Agentdesk-macos-arm64.dmg"
  end
  on_intel do
    url "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-workflows-latest/Agentdesk-macos-x64.dmg"
  end

  name "Agentdesk"
  desc "Desktop dashboard for agentd runs"
  homepage "https://github.com/beagle-tools/beagle-tools/tree/main/workflows/desktop"

  app "Agentdesk.app"

  # Not notarized yet — clear quarantine so first launch works.
  postflight do
    system_command "/usr/bin/xattr",
                   args:  ["-dr", "com.apple.quarantine", "#{appdir}/Agentdesk.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.matthewmyrick.agentdesk",
    "~/Library/Caches/com.matthewmyrick.agentdesk",
    "~/Library/Preferences/com.matthewmyrick.agentdesk.plist",
    "~/Library/Saved Application State/com.matthewmyrick.agentdesk.savedState",
    "~/Library/WebKit/com.matthewmyrick.agentdesk",
  ]
end
