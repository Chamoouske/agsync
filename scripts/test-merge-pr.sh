#!/usr/bin/env bash
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/merge-pr.sh"
work=$(mktemp -d)

cat > "$work/gh" <<'EOF'
#!/usr/bin/env bash
echo "gh $*" >> "$GH_CALLS"
if [ "$1 $2" = "pr view" ]; then
  echo abc123
  exit 0
fi
reply=$(head -n 1 "$GH_REPLIES")
sed -i '1d' "$GH_REPLIES"
echo "${reply#*|}" >&2
exit "${reply%%|*}"
EOF
chmod +x "$work/gh"

export PATH="$work:$PATH" GH_CALLS="$work/calls" GH_REPLIES="$work/replies"
export MERGE_PR_ATTEMPTS=3 MERGE_PR_DELAY=0

clean='1|GraphQL: Pull request Pull request is in clean status (enablePullRequestAutoMerge)'
queued='1|GraphQL: Repository rule violations found 2 of 2 required status checks are queued. (mergePullRequest)'
other='1|GraphQL: Resource not accessible by personal access token'
ok='0|done'

failures=0

expect() {
  local name=$1 want=$2 calls=$3 got
  shift 3
  printf '%s\n' "$@" > "$GH_REPLIES"
  : > "$GH_CALLS"
  if "$script" https://example.test/pull/1 > /dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ] && [ "$(grep -c 'pr merge' "$GH_CALLS")" = "$calls" ]; then
    echo "ok   $name: $got"
  else
    echo "FAIL $name: want $want with $calls merge calls, got $got with:"
    cat "$GH_CALLS"
    failures=$((failures + 1))
  fi
}

expect "auto-merge accepted" pass 1 "$ok"
expect "clean, then direct merge" pass 2 "$clean" "$ok"
grep -q -- '--merge --match-head-commit abc123' "$GH_CALLS" || { echo "FAIL direct merge is not pinned to the head commit"; failures=$((failures + 1)); }
expect "clean, direct merge blocked by queued checks, then auto-merge" pass 3 "$clean" "$queued" "$ok"
expect "gh merges directly itself and hits queued checks, then auto-merge" pass 2 "$queued" "$ok"
expect "other error on auto-merge fails" fail 1 "$other"
expect "clean, then other error on direct merge fails" fail 2 "$clean" "$other"
expect "queued checks on every attempt fails" fail 6 "$clean" "$queued" "$clean" "$queued" "$clean" "$queued"

rm -rf "$work"
exit "$failures"
