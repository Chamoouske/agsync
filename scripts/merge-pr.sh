#!/usr/bin/env bash
set -euo pipefail

url=$1
attempts=${MERGE_PR_ATTEMPTS:-6}
delay=${MERGE_PR_DELAY:-10}

for attempt in $(seq "$attempts"); do
  if out=$(gh pr merge "$url" --auto --merge 2>&1); then
    echo "$out"
    exit 0
  fi
  echo "$out"

  if grep -q 'clean status' <<< "$out"; then
    echo "$url is already mergeable, so auto-merge has nothing to wait for; merging it now"
    head=$(gh pr view "$url" --json headRefOid --jq .headRefOid)
    if out=$(gh pr merge "$url" --merge --match-head-commit "$head" 2>&1); then
      echo "$out"
      exit 0
    fi
    echo "$out"
  fi

  if ! grep -q 'required status check' <<< "$out"; then
    echo "::error::could not merge $url or turn on its auto-merge"
    exit 1
  fi
  echo "attempt $attempt of $attempts: the required checks of $url were just queued; trying auto-merge again in ${delay}s"
  sleep "$delay"
done

echo "::error::$url still had no auto-merge after $attempts attempts"
exit 1
