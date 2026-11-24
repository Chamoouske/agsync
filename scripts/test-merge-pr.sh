#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/merge-pr.sh"
work=$(mktemp -d)

cat > "$work/gh" <<'EOF'
#!/usr/bin/env bash
echo "gh $*" >> "$GH_CALLS"
if [ "$1 $2" = "pr view" ]; then
  case "$*" in
    *mergeStateStatus*) echo "$GH_MERGE_STATE" ;;
    *headRefOid*) echo abc123 ;;
    *state*) echo "$GH_PR_STATE" ;;
  esac
  exit 0
fi
reply=$(head -n 1 "$GH_REPLIES")
sed -i '1d' "$GH_REPLIES"
if [ -n "${reply#*|}" ]; then
  echo "${reply#*|}" >&2
fi
exit "${reply%%|*}"
EOF
chmod +x "$work/gh"

export PATH="$work:$PATH" GH_CALLS="$work/calls" GH_REPLIES="$work/replies" GH_MERGE_STATE=CLEAN GH_PR_STATE=OPEN
export GH_TOKEN=fake-secret-token
export MERGE_PR_ATTEMPTS=3 MERGE_PR_DELAY=0

clean='1|GraphQL: Pull request Pull request is in clean status (enablePullRequestAutoMerge)'
queued='1|GraphQL: Repository rule violations found 2 of 2 required status checks are queued. (mergePullRequest)'
other='1|GraphQL: Resource not accessible by personal access token'
ok='0|'

url=https://example.test/pull/1
failures=0

expect() {
  local name=$1 want=$2 calls=$3 got
  shift 3
  printf '%s\n' "$@" > "$GH_REPLIES"
  : > "$GH_CALLS"
  if "$script" "$url" > "$work/log" 2>&1; then got=pass; else got=fail; fi
  cat "$work/log" >> "$work/all"
  if [ "$got" = "$want" ] && [ "$(grep -c 'pr merge' "$GH_CALLS")" = "$calls" ]; then
    echo "ok   $name: $got"
  else
    echo "FAIL $name: want $want with $calls merge calls, got $got with:"
    cat "$GH_CALLS"
    failures=$((failures + 1))
  fi
}

expect_log() {
  if grep -qF -- "$1" "$work/log"; then
    echo "ok     log has: $1"
  else
    echo "FAIL   log is missing: $1"
    cat "$work/log"
    failures=$((failures + 1))
  fi
}

expect "auto-merge accepted" pass 1 "$ok"
expect_log "::group::merge attempt 1 of 3 for $url"
expect_log "mergeStateStatus: CLEAN"
expect_log "running: gh pr merge $url --auto --merge"
expect_log "GitHub answered (exit 0):"
expect_log "(no output)"
expect_log "::endgroup::"
expect_log "::notice::auto-merge is on for $url (attempt 1)"

GH_PR_STATE=MERGED expect "gh merges directly itself" pass 1 "$ok"
expect_log "::notice::$url was merged directly on attempt 1"

expect "clean, then direct merge" pass 2 "$clean" "$ok"
expect_log "GitHub answered (exit 1):"
expect_log "Pull request is in clean status"
expect_log "running: gh pr merge $url --merge --match-head-commit abc123"
expect_log "::notice::$url was merged directly at abc123 on attempt 1"

expect "clean, direct merge blocked by queued checks, then auto-merge" pass 3 "$clean" "$queued" "$ok"
expect_log "2 of 2 required status checks are queued"
expect_log "waiting 0s before trying auto-merge again"
expect_log "::group::merge attempt 2 of 3 for $url"
expect_log "::notice::auto-merge is on for $url (attempt 2)"

expect "gh merges directly itself and hits queued checks, then auto-merge" pass 2 "$queued" "$ok"

expect "other error on auto-merge fails" fail 1 "$other"
expect_log "Resource not accessible by personal access token"
expect_log "::error::could not merge $url or turn on its auto-merge on attempt 1"

expect "clean, then other error on direct merge fails" fail 2 "$clean" "$other"

expect "queued checks on every attempt fails" fail 6 "$clean" "$queued" "$clean" "$queued" "$clean" "$queued"
expect_log "::error::$url still had no auto-merge after 3 attempts"

if grep -q "$GH_TOKEN" "$work/all"; then
  echo "FAIL log mentions the token"
  failures=$((failures + 1))
fi

rm -rf "$work"
exit "$failures"
