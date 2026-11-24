#!/usr/bin/env bash
# Capture unmodified startup windows in independent, clean X11 sessions.
set -euo pipefail
if (( $# != 3 )); then
  echo "Usage: $0 T3_APPIMAGE_EXTRACTED_DIR BUTLER_BINARY OUTPUT_DIR" >&2
  exit 2
fi
t3=$(realpath "$1")
butler=$(realpath "$2")
out=$(realpath -m "$3")
width=1280
height=800
mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

if command -v magick >/dev/null; then
  capture=(magick import)
  identify=(magick identify)
  compare=(magick compare)
else
  capture=(import)
  identify=(identify)
  compare=(compare)
fi

capture_one() (
  set -euo pipefail
  local name=$1; shift
  local runtime="$tmp/$name" window="" tick dimensions
  mkdir -p -m 700 "$runtime"/{config,cache,data,state,run}
  XDG_CONFIG_HOME="$runtime/config" XDG_CACHE_HOME="$runtime/cache" \
    XDG_DATA_HOME="$runtime/data" XDG_STATE_HOME="$runtime/state" \
    XDG_RUNTIME_DIR="$runtime/run" "$@" >"$out/$name.log" 2>&1 &
  local pid=$!
  trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT

  # Both toolkits set _NET_WM_PID on their top-level client; ignore helper windows.
  for ((tick=0; tick<90; tick++)); do
    kill -0 "$pid" 2>/dev/null || { cat "$out/$name.log" >&2; return 1; }
    while read -r id; do
      local geometry
      geometry=$(xdotool getwindowgeometry --shell "$id" 2>/dev/null) || continue
      if grep -Eq '^WIDTH=([5-9][0-9]{2}|[0-9]{4,})$' <<<"$geometry" &&
         grep -Eq '^HEIGHT=([3-9][0-9]{2}|[0-9]{4,})$' <<<"$geometry"; then
        window=$id
        break
      fi
    done < <(xdotool search --onlyvisible --pid "$pid" 2>/dev/null || true)
    [[ -z "$window" ]] || break
    sleep 1
  done
  if [[ -z "$window" ]]; then
    xwininfo -root -tree >&2 || true
    cat "$out/$name.log" >&2
    echo "No visible client window for $name (pid $pid)" >&2
    return 1
  fi

  xdotool windowmove "$window" 0 0 windowsize "$window" "$width" "$height"
  xdotool mousemove 1390 890
  sleep 3
  "${capture[@]}" -window "$window" "$out/$name-startup.png"
  dimensions=$("${identify[@]}" -format '%wx%h' "$out/$name-startup.png")
  if [[ "$dimensions" != "${width}x${height}" ]]; then
    echo "$name client is $dimensions, expected ${width}x${height}" >&2
    return 1
  fi
  local colors
  colors=$("${identify[@]}" -format '%k' "$out/$name-startup.png")
  if (( colors < 10 )); then
    echo "$name capture is blank or did not finish rendering" >&2
    return 1
  fi
)

capture_one t3 "$t3/AppRun"
capture_one butler "$butler"
rc=0
"${compare[@]}" -metric AE "$out/t3-startup.png" "$out/butler-startup.png" \
  "$out/startup-diff.png" 2>"$out/differing-pixels.txt" || rc=$?
if (( rc > 1 )); then
  cat "$out/differing-pixels.txt" >&2
  exit "$rc"
fi
# Startup defaults are diagnostic, not a parity check on a matched fixture.
printf 'Native startup differing pixels: %s\n' "$(cat "$out/differing-pixels.txt")"
