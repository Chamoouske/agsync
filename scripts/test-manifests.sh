#!/usr/bin/env bash
set -euo pipefail

scripts="$(cd "$(dirname "$0")" && pwd)"
packaging="$scripts/../packaging"
work=$(mktemp -d)
root=$(cd "$work" && (pwd -W 2>/dev/null || pwd))
asset=agsync-x86_64-pc-windows-msvc.exe

export AGSYNC_PACKAGING_DIR="$work/packaging"
export AGSYNC_RELEASE_BASE_URL="file://$root/releases"

failures=0

reset() {
  rm -rf "$work/packaging"
  cp -r "$packaging" "$work/packaging"
  "$scripts/update-manifests.sh" 9.8.7 "$work/SHA256SUMS" > /dev/null
}

expect() {
  local name=$1 want=$2
  shift 2
  if "$@" > /dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ]; then
    echo "ok   $name: $got"
  else
    echo "FAIL $name: want $want got $got"
    failures=$((failures + 1))
  fi
}

mkdir -p "$work/releases/v9.8.7"
printf 'fake binary' > "$work/releases/v9.8.7/$asset"
(cd "$work/releases/v9.8.7" && sha256sum "$asset" > "$work/SHA256SUMS")

reset
expect "updated manifests are consistent" pass "$scripts/check-manifests.sh" "$work/SHA256SUMS"
expect "hash matches the published asset" pass "$scripts/check-manifests.sh" --remote
expect "autoupdate url keeps \$version" pass grep -q 'download/v\$version/' "$work/packaging/scoop/agsync.json"
expect "installer hash is uppercase" pass grep -q "InstallerSha256: $(cut -c1-64 "$work/SHA256SUMS" | tr a-f A-F)" "$work/packaging/winget/Chamoouske.agsync.installer.yaml"

sed -i 's/^PackageVersion: .*/PackageVersion: 9.8.6/' "$work/packaging/winget/Chamoouske.agsync.locale.en-US.yaml"
expect "stale winget version" fail "$scripts/check-manifests.sh"

reset
sed -i 's|/v9.8.7/|/v9.8.6/|' "$work/packaging/winget/Chamoouske.agsync.installer.yaml"
expect "winget url of another version" fail "$scripts/check-manifests.sh"

reset
sed -i 's|#/agsync.exe|#/other.exe|' "$work/packaging/scoop/agsync.json"
expect "scoop url with wrong rename" fail "$scripts/check-manifests.sh"

reset
sed -i -E 's/^( *InstallerSha256: ).*/\1'"$(printf 'A%.0s' {1..64})"'/' "$work/packaging/winget/Chamoouske.agsync.installer.yaml"
expect "winget hash diverges from scoop" fail "$scripts/check-manifests.sh"

reset
printf '%064d  %s\n' 0 "$asset" > "$work/OTHER"
expect "hash diverges from SHA256SUMS" fail "$scripts/check-manifests.sh" "$work/OTHER"

printf 'tampered' > "$work/releases/v9.8.7/$asset"
expect "hash diverges from the published asset" fail "$scripts/check-manifests.sh" --remote

rm "$work/releases/v9.8.7/$asset"
expect "published asset is missing" fail "$scripts/check-manifests.sh" --remote

expect "SHA256SUMS without the asset" fail "$scripts/update-manifests.sh" 1.0.0 /dev/null

rm -rf "$work"
exit "$failures"
