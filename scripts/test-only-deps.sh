#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/only-deps.sh"
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

expect "one cargo bump" 0 "chore(deps): bump serde from 1.0.229 to 1.0.230"
expect "grouped cargo and actions" 0 "$(printf 'chore(deps): bump the cargo-minor-patch group with 3 updates\nci(deps): bump actions/checkout from 4.2.2 to 5.0.0')"
expect "dev dependency" 0 "chore(deps-dev): bump tempfile from 3.0.0 to 3.1.0"
expect "deps with a sync commit" 0 "$(printf 'chore: sync stage with master\nchore(deps): bump toml from 1.2.1 to 1.2.2')"
expect "deps with a feat" 1 "$(printf 'chore(deps): bump toml from 1.2.1 to 1.2.2\nfeat(cli): add a flag')"
expect "deps with a fix" 1 "$(printf 'fix: small thing\nci(deps): bump actions/checkout from 4.2.2 to 4.2.3')"
expect "deps with docs" 1 "$(printf 'chore(deps): bump toml from 1.2.1 to 1.2.2\ndocs: words')"
expect "other chore scope" 1 "chore(release): changelog and manifests for v1.0.0"
expect "breaking deps bump promotes" 1 "chore(deps)!: bump windows from 0.61.3 to 1.0.0"
expect "only a sync commit promotes" 1 "chore: sync stage with master"
expect "no commits promotes" 1 ""

exit "$failures"
