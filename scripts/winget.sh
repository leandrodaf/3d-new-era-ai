#!/usr/bin/env bash
# Writes the winget manifests of a release, for the first submission to
# microsoft/winget-pkgs (later releases are sent by the release job).
#
#   scripts/winget.sh 1.7.0 <sha256 of newera-windows-x64.zip> [out-dir]
#
# The zip is portable: winget unpacks it and puts `newera` and `newera-gui`
# on the PATH, no installer and no administrator.
set -euo pipefail

version="${1:?version}"
sha="$(printf '%s' "${2:?sha256 of newera-windows-x64.zip}" | tr 'a-f' 'A-F')"
id="LeandroFerreira.3DNewEraAI"
out="${3:-target/packaging/winget}/manifests/l/LeandroFerreira/3DNewEraAI/$version"
url="https://github.com/leandrodaf/3d-new-era-ai/releases/download/v$version/newera-windows-x64.zip"
mkdir -p "$out"

cat > "$out/$id.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.10.0.schema.json
PackageIdentifier: $id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.10.0
YAML

cat > "$out/$id.installer.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.10.0.schema.json
PackageIdentifier: $id
PackageVersion: $version
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
  - RelativeFilePath: 3D New Era AI\\newera.exe
    PortableCommandAlias: newera
  - RelativeFilePath: 3D New Era AI\\newera-gui.exe
    PortableCommandAlias: newera-gui
Installers:
  - Architecture: x64
    InstallerUrl: $url
    InstallerSha256: $sha
ManifestType: installer
ManifestVersion: 1.10.0
YAML

cat > "$out/$id.locale.en-US.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.1.10.0.schema.json
PackageIdentifier: $id
PackageVersion: $version
PackageLocale: en-US
Publisher: Leandro Ferreira
PublisherUrl: https://github.com/leandrodaf
PublisherSupportUrl: https://github.com/leandrodaf/3d-new-era-ai/issues
PrivacyUrl: https://3dneweraai.com/privacy/
PackageName: 3D New Era AI
PackageUrl: https://3dneweraai.com
License: MIT OR Apache-2.0
LicenseUrl: https://github.com/leandrodaf/3d-new-era-ai#license
ShortDescription: Design your home by talking to your AI, and see every change on screen.
Description: |-
  3D New Era AI is a free home design app. Draw the floor plan, furnish the rooms and plan kitchens
  and wardrobes yourself, or ask Claude, ChatGPT or another AI to do it for you: every change shows
  up on your screen as it happens and is one Ctrl+Z away. See the result as a plan, a 3D view or a
  realistic photo. Your projects stay on your computer; no account needed.
Moniker: newera
Tags:
  - architecture
  - cad
  - floor-plan
  - home-design
  - interior-design
  - mcp
ReleaseNotesUrl: https://github.com/leandrodaf/3d-new-era-ai/releases/tag/v$version
ManifestType: defaultLocale
ManifestVersion: 1.10.0
YAML
echo "$out"
