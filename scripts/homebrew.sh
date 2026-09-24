#!/usr/bin/env bash
# Writes the Homebrew cask of a release, for the tap leandrodaf/homebrew-tap
# (`brew install leandrodaf/tap/3d-new-era-ai`). The release job commits it
# there; run by hand for the first one.
#
#   scripts/homebrew.sh 1.7.0 <sha256 apple-silicon zip> <sha256 intel zip> [out]
set -euo pipefail

version="${1:?version}"
arm="${2:?sha256 of newera-macos-apple-silicon.zip}"
intel="${3:?sha256 of newera-macos-intel.zip}"
out="${4:-packaging/homebrew/Casks/3d-new-era-ai.rb}"
mkdir -p "$(dirname "$out")"

cat > "$out" <<RUBY
cask "3d-new-era-ai" do
  arch arm: "apple-silicon", intel: "intel"

  version "$version"
  sha256 arm:   "$arm",
         intel: "$intel"

  url "https://github.com/leandrodaf/3d-new-era-ai/releases/download/v#{version}/newera-macos-#{arch}.zip"
  name "3D New Era AI"
  desc "Open-source home design editor with a built-in MCP server for AI agents"
  homepage "https://3dneweraai.com/"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "3D New Era AI.app"
  binary "#{appdir}/3D New Era AI.app/Contents/MacOS/newera"

  zap trash: [
    "~/.cache/3d-new-era-ai",
    "~/Library/Application Support/3d-new-era-ai",
  ]

  caveats <<~EOS
    The app is not notarized by Apple. If macOS refuses to open it the first time:
      xattr -dr com.apple.quarantine "#{appdir}/3D New Era AI.app"

    Connect your AI: https://github.com/leandrodaf/3d-new-era-ai#connect-your-ai
  EOS
end
RUBY
echo "$out"
