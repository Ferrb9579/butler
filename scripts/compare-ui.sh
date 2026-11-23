#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 4 || ! $1 =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
  echo "Usage: $0 SCREEN-STATE T3_URL T3_READY_SELECTOR BUTLER_X11_WINDOW_ID" >&2
  exit 2
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
reference=${T3_REFERENCE_DIR:-"$script_dir/../../t3code-reference"}
output=${UI_OUTPUT_DIR:-"$script_dir/../artifacts/ui"}
screenshots="$script_dir/../screenshots"
theme=${UI_THEME:-dark}
if [[ $theme != dark && $theme != light ]]; then
  echo "UI_THEME must be dark or light" >&2
  exit 2
fi

mkdir -p "$output"
capture=$(mktemp -d "$output/.capture.XXXXXXXX")
trap 'rm -rf -- "$capture"' EXIT
magick import -window "$4" "$capture/butler.png"
read -r width height < <(magick identify -format '%w %h\n' "$capture/butler.png")
node "$script_dir/capture-reference.mjs" "$reference" "$2" "$3" "$width" "$height" "$capture/t3.png"

name="$1-$theme-${width}x${height}"
result=0
magick compare -metric AE "$capture/t3.png" "$capture/butler.png" "$capture/diff.png" \
  2>"$capture/pixel-count.txt" || result=$?
if (( result > 1 )); then
  cat "$capture/pixel-count.txt" >&2
  exit "$result"
fi

mv -- "$capture/t3.png" "$screenshots/t3/$name.png"
mv -- "$capture/butler.png" "$screenshots/butler/$name.png"
mv -- "$capture/diff.png" "$output/$name-diff.png"
mv -- "$capture/pixel-count.txt" "$output/$name-pixel-count.txt"
cat "$output/$name-pixel-count.txt"
printf '\nScreenshot pair: %s.png\n' "$name"
exit "$result"
