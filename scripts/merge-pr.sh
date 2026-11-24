#!/usr/bin/env bash
set -euo pipefail

url=$1
attempts=${MERGE_PR_ATTEMPTS:-6}
delay=${MERGE_PR_DELAY:-10}

pr_field() {
  gh pr view "$url" --json "$1" --jq ".$1" 2>/dev/null || echo "unknown"
}

log_state() {
  echo "mergeStateStatus: $(pr_field mergeStateStatus)"
}

run_merge() {
  echo "running: gh pr merge $url $*"
  local status=0
  out=$(gh pr merge "$url" "$@" 2>&1) || status=$?
  echo "GitHub answered (exit $status):"
  echo "${out:-(no output)}"
  return "$status"
}

finish_merged_or_auto() {
  echo "::endgroup::"
  if [ "$(pr_field state)" = "MERGED" ]; then
    echo "::notice::$url was merged directly on attempt $attempt"
  else
    echo "::notice::auto-merge is on for $url (attempt $attempt)"
  fi
  exit 0
}

fail() {
  echo "::endgroup::"
  echo "::error::$1"
  exit 1
}

for attempt in $(seq "$attempts"); do
  echo "::group::merge attempt $attempt of $attempts for $url"
  log_state
  if run_merge --auto --merge; then
    finish_merged_or_auto
  fi

  if grep -q 'clean status' <<< "$out"; then
    echo "the pull request is already mergeable, so auto-merge has nothing to wait for; merging it directly"
    log_state
    head=$(pr_field headRefOid)
    if run_merge --merge --match-head-commit "$head"; then
      echo "::endgroup::"
      echo "::notice::$url was merged directly at $head on attempt $attempt"
      exit 0
    fi
  fi

  if ! grep -q 'required status check' <<< "$out"; then
    fail "could not merge $url or turn on its auto-merge on attempt $attempt"
  fi
  echo "::endgroup::"
  if [ "$attempt" -eq "$attempts" ]; then
    break
  fi
  echo "the required checks of $url were just queued; waiting ${delay}s before trying auto-merge again"
  sleep "$delay"
done

echo "::error::$url still had no auto-merge after $attempts attempts"
exit 1
