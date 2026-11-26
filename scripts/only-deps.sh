#!/usr/bin/env bash
set -euo pipefail

deps='^(chore|ci)\(deps(-dev)?\): [^ ]'
sync='^chore: sync stage with master$'

deps_count=0
other_count=0
while IFS= read -r line || [ -n "$line" ]; do
  if [ -z "$line" ] || grep -Eq "$sync" <<< "$line"; then
    continue
  fi
  if grep -Eq "$deps" <<< "$line"; then
    deps_count=$((deps_count + 1))
  else
    other_count=$((other_count + 1))
  fi
done

echo "dependency commits: $deps_count, other commits: $other_count"

if [ "$deps_count" -gt 0 ] && [ "$other_count" -eq 0 ]; then
  exit 0
fi
exit 1
