cask "3d-new-era-ai" do
  arch arm: "apple-silicon", intel: "intel"

  version "1.7.0"
  sha256 arm:   "640bac45e299297347085bdf87a1325a6545754bd6d4ffaf6144560fdf184351",
         intel: "3df7f6aa8df29bcb1a01d7cf946baa7d26728b244d8587d7425e3edc23f60ad6"

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
