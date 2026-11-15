#!/bin/sh
# Move an md-kanban card: card.sh <id-substring> <status>
set -e
f=$(find "$(dirname "$0")/../../s-md-slides-tasks" -name "*$1*.md" | head -1)
[ -n "$f" ] || { echo "no card matches $1" >&2; exit 1; }
now=$(date -u +%Y-%m-%dT%H:%M:%S.000Z)
sed -i '' -e "s/^status: \".*\"/status: \"$2\"/" -e "s/^modified: \".*\"/modified: \"$now\"/" "$f"
echo "$f -> $2"
