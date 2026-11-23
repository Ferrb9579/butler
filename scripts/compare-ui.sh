#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 3 ]]; then
  echo "Usage: $0 T3_URL T3_READY_SELECTOR BUTLER_X11_WINDOW_ID" >&2
  exit 2
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
reference=${T3_REFERENCE_DIR:-"$script_dir/../../t3code-reference"}
output=${UI_OUTPUT_DIR:-"$script_dir/../artifacts/ui"}
mkdir -p "$output"
rm -f -- "$output/diff.png" "$output/pixel-count.txt"

magick import -window "$3" "$output/butler.png"
read -r width height < <(magick identify -format '%w %h\n' "$output/butler.png")
node "$script_dir/capture-reference.mjs" "$reference" "$1" "$2" "$width" "$height" "$output/t3.png"

result=0
magick compare -metric AE "$output/t3.png" "$output/butler.png" "$output/diff.png" \
  2>"$output/pixel-count.txt" || result=$?
cat "$output/pixel-count.txt"
printf '\nArtifacts: %s\n' "$output"
exit "$result"
