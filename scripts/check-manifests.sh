#!/usr/bin/env bash
set -euo pipefail

remote=0
if [ "${1:-}" = "--remote" ]; then
  remote=1
  shift
fi
sums=${1:-}
dir=${AGSYNC_PACKAGING_DIR:-packaging}
base=${AGSYNC_RELEASE_BASE_URL:-https://github.com/Chamoouske/agsync/releases/download}
asset=agsync-x86_64-pc-windows-msvc.exe
scoop="$dir/scoop/agsync.json"
installer="$dir/winget/Chamoouske.agsync.installer.yaml"
failures=0

fail() {
  echo "FAIL $1" >&2
  failures=$((failures + 1))
}

field() {
  tr -d '\r' < "$2" | sed -n -E "$1" | head -n 1
}

version=$(field 's/^    "version": "([^"]*)",?$/\1/p' "$scoop")
scoop_url=$(field 's/^ *"url": "([^"$]*)",?$/\1/p' "$scoop")
scoop_hash=$(field 's/^ *"hash": "([^"]*)",?$/\1/p' "$scoop")
winget_url=$(field 's/^ *InstallerUrl: (.*)$/\1/p' "$installer")
winget_hash=$(field 's/^ *InstallerSha256: (.*)$/\1/p' "$installer")
url="$base/v$version/$asset"

if ! grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' <<< "$version"; then
  fail "versao do Scoop invalida: '$version'"
fi

for f in "$dir"/winget/*.yaml; do
  got=$(field 's/^PackageVersion: (.*)$/\1/p' "$f")
  id=$(field 's/^PackageIdentifier: (.*)$/\1/p' "$f")
  [ "$got" = "$version" ] || fail "$f tem PackageVersion $got, Scoop tem $version"
  [ "$id" = "Chamoouske.agsync" ] || fail "$f tem PackageIdentifier $id"
done

[ "$scoop_url" = "$url#/agsync.exe" ] || fail "URL do Scoop $scoop_url nao e $url#/agsync.exe"
[ "$winget_url" = "$url" ] || fail "InstallerUrl do WinGet $winget_url nao e $url"

if ! grep -Eq '^[0-9a-f]{64}$' <<< "$scoop_hash"; then
  fail "hash do Scoop invalido: '$scoop_hash'"
fi
[ "$winget_hash" = "$(tr a-f A-F <<< "$scoop_hash")" ] || fail "InstallerSha256 $winget_hash diverge do hash do Scoop $scoop_hash"

if [ -n "$sums" ]; then
  expected=$(tr -d '\r' < "$sums" | awk -v a="$asset" '$2 == a || $2 == "*" a { print tolower($1) }')
  [ "$scoop_hash" = "$expected" ] || fail "hash $scoop_hash diverge do $sums ($expected)"
fi

if [ "$remote" = 1 ]; then
  if published=$(curl -fsSL "$url" | sha256sum | cut -d ' ' -f 1); then
    [ "$scoop_hash" = "$published" ] || fail "hash $scoop_hash diverge do artefato publicado em $url ($published)"
  else
    fail "nao consegui baixar $url"
  fi
fi

if [ "$failures" -eq 0 ]; then
  echo "ok   manifests de v$version consistentes"
fi
exit "$failures"
