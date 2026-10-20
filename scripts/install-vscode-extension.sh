#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/extension"
cp "$root/integrations/vscode/package.json" "$root/integrations/vscode/extension.js" "$root/integrations/vscode/README.md" "$tmp/extension/"
cat > "$tmp/extension.vsixmanifest" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Id="lyapunov-workspace" Version="0.1.0" Publisher="chaos-tic" Language="en-US"/>
    <DisplayName>Lyapunov Workspace</DisplayName>
    <Description>Reports the current local VS Code project to Lyapunov.</Description>
  </Metadata>
  <Installation><InstallationTarget Id="Microsoft.VisualStudio.Code"/></Installation>
  <Dependencies/>
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json"/>
    <Asset Type="Microsoft.VisualStudio.Services.Content.Details" Path="extension/README.md"/>
  </Assets>
</PackageManifest>
XML
cat > "$tmp/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="js" ContentType="application/javascript"/>
  <Default Extension="md" ContentType="text/markdown"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
</Types>
XML
(cd "$tmp" && zip -q -r lyapunov-workspace.vsix '[Content_Types].xml' extension.vsixmanifest extension)
code --install-extension "$tmp/lyapunov-workspace.vsix" --force
echo 'Reload VS Code windows to activate Lyapunov Workspace.'
