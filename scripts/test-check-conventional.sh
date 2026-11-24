#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/check-conventional.sh"
failures=0

expect() {
  local name=$1 want=$2 input=$3 got=0
  printf '%s\n' "$input" | "$script" > /dev/null || got=$?
  if [ "$got" = "$want" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: want exit $want got $got"
    failures=$((failures + 1))
  fi
}

expect "plain type" 0 "fix: small thing"
expect "scope" 0 "ci(docs): deploy the book"
expect "breaking" 0 "feat(cli)!: drop old flag"
expect "several valid lines" 0 "$(printf 'docs: words\nchore(release): changelog and manifests for v1.0.0')"
expect "no type" 1 "Initial commit"
expect "unknown type" 1 "feature: new flag"
expect "missing space" 1 "fix:thing"
expect "uppercase type" 1 "Fix: thing"
expect "one bad line fails" 1 "$(printf 'fix: ok\nwip')"

exit "$failures"
