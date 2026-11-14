#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/next-version.sh"
repo=$(mktemp -d)
cd "$repo"

git init -q
git config user.email test@example.com
git config user.name test
printf '[package]\nname = "x"\nversion = "0.1.0"\n' > Cargo.toml
git add Cargo.toml
git commit -q -m "chore: start"

failures=0

expect() {
  local name=$1 want=$2 got
  got=$("$script")
  if [ "$got" = "$want" ]; then
    echo "ok   $name: $got"
  else
    echo "FAIL $name: want $want got $got"
    failures=$((failures + 1))
  fi
}

expect "no tag uses Cargo.toml" 0.1.0

git tag v1.2.3
git commit -q --allow-empty -m "fix: small thing"
git commit -q --allow-empty -m "docs: words"
expect "patch" 1.2.4

git commit -q --allow-empty -m "feat(cli): new flag"
expect "feat is minor" 1.3.0

git commit -q --allow-empty -m "feat!: drop old flag"
expect "bang is major" 2.0.0

git reset -q --hard v1.2.3
git commit -q --allow-empty -m "fix: thing" -m "BREAKING CHANGE: output format changed"
expect "footer is major" 2.0.0

rm -rf "$repo"
exit "$failures"
