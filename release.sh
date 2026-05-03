#!/bin/bash
set -e

LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null)

if [ -z "$LAST_TAG" ]; then
    echo "No previous release tag found. Skipping squash."
    exec cargo release -x "$@"
fi

COMMIT_COUNT=$(git rev-list "$LAST_TAG"..HEAD --count)

if [ "$COMMIT_COUNT" -le 1 ]; then
    echo "Only $COMMIT_COUNT commit(s) since $LAST_TAG. No squash needed."
    exec cargo release -x "$@"
fi

echo "Found $COMMIT_COUNT commits since $LAST_TAG:"
echo
git log "$LAST_TAG"..HEAD --oneline
echo
echo -n "Squash these $COMMIT_COUNT commits before releasing? [y/N] "
read -r CONFIRM

if [ "$CONFIRM" != "y" ] && [ "$CONFIRM" != "Y" ]; then
    echo "Skipping squash. Proceeding with release."
    exec cargo release -x "$@"
fi

git reset --soft HEAD~"$COMMIT_COUNT"
git commit -m "prep for release"

echo "Squashed $COMMIT_COUNT commits into one."
exec cargo release -x "$@"
