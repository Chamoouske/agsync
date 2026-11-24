#!/usr/bin/env bash
set -euo pipefail

pattern='^(feat|fix|perf|refactor|docs|ci|build|chore|test|style|revert|release)(\([a-z0-9._/-]+\))?!?: [^ ]'

bad=0
while IFS= read -r line || [ -n "$line" ]; do
  if grep -Eq "$pattern" <<< "$line"; then
    continue
  fi
  echo "::error::not a Conventional Commit: $line"
  bad=$((bad + 1))
done

if [ "$bad" -gt 0 ]; then
  echo "use tipo(escopo): descricao, with tipo one of feat, fix, perf, refactor, docs, ci, build, chore, test, style, revert, release"
  exit 1
fi
