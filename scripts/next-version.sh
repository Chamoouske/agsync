#!/usr/bin/env bash
set -euo pipefail

tag=$(git tag --list 'v*' --sort=-v:refname | head -n 1)

if [ -z "$tag" ]; then
  sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1
  exit 0
fi

subjects=$(git log "$tag..HEAD" --format=%s)
bodies=$(git log "$tag..HEAD" --format=%b)

IFS=. read -r major minor patch <<< "${tag#v}"

if grep -Eq '^[a-z]+(\([^)]*\))?!:' <<< "$subjects" || grep -Eq '^BREAKING[ -]CHANGE:' <<< "$bodies"; then
  echo "$((major + 1)).0.0"
elif grep -Eq '^feat(\([^)]*\))?:' <<< "$subjects"; then
  echo "$major.$((minor + 1)).0"
else
  echo "$major.$minor.$((patch + 1))"
fi
