#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "uso: $0 <versao> <SHA256SUMS>" >&2
  exit 2
fi

version=$1
sums=$2
dir=${AGSYNC_PACKAGING_DIR:-packaging}
base=${AGSYNC_RELEASE_BASE_URL:-https://github.com/Chamoouske/agsync/releases/download}
asset=agsync-x86_64-pc-windows-msvc.exe

hash=$(tr -d '\r' < "$sums" | awk -v a="$asset" '$2 == a || $2 == "*" a { print tolower($1) }')
if ! grep -Eq '^[0-9a-f]{64}$' <<< "$hash"; then
  echo "$sums nao tem um SHA256 para $asset" >&2
  exit 1
fi

url="$base/v$version/$asset"
upper=$(tr a-f A-F <<< "$hash")

sed -i -E \
  -e "s|^    \"version\": .*|    \"version\": \"$version\",|" \
  -e "s|^( *)\"url\": \"[^\"$]*\",?\r?$|\1\"url\": \"$url#/agsync.exe\",|" \
  -e "s|^( *)\"hash\": .*|\1\"hash\": \"$hash\"|" \
  "$dir/scoop/agsync.json"

for f in "$dir"/winget/*.yaml; do
  sed -i -E "s|^PackageVersion: .*|PackageVersion: $version|" "$f"
done

sed -i -E \
  -e "s|^( *)InstallerUrl: .*|\1InstallerUrl: $url|" \
  -e "s|^( *)InstallerSha256: .*|\1InstallerSha256: $upper|" \
  "$dir/winget/Chamoouske.agsync.installer.yaml"

echo "manifests em $dir apontam para v$version ($hash)"
