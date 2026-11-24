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
  transform=(magick)
  capture=(magick import)
  identify=(magick identify)
  compare=(magick compare)
else
  transform=(convert)
  capture=(import)
  identify=(identify)
  compare=(compare)
fi

capture_one() (
  set -euo pipefail
  local name=$1; shift
  local profile=${CAPTURE_PROFILE:-$name}
  local runtime="$tmp/$profile" window="" tick dimensions
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
  if [[ "$name" == t3 || "$name" == butler ]]; then
    await_text "$window" "$out/$name-startup.png" 'Start without an agent' '240x42+270+601'
  elif [[ "$name" == butler-project-reopened ]]; then
    await_text "$window" "$out/$name-startup.png" 'ParityFixture'
  else
    await_text "$window" "$out/$name-startup.png" 'What should we work on' '465x103+535+355'
  fi
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
  if [[ "$name" == t3 && "${CAPTURE_REFERENCE_LANDING:-0}" == 1 ]]; then
    xdotool windowfocus "$window" mousemove --window "$window" 365 620 click 1
    xdotool mousemove 1390 890
    if await_text "$window" "$out/t3-after-welcome.png" 'what should we work on' '465x103+535+355'; then
      xdotool windowfocus "$window" mousemove --window "$window" 670 462 click 1
      xdotool mousemove 1390 890
      if await_text "$window" "$out/t3-add-project.png" 'Local folder'; then
        xdotool windowfocus "$window" mousemove --window "$window" 438 230 click 1
        xdotool mousemove 1390 890
        sleep 2
        "${capture[@]}" -window "$window" "$out/t3-local-folder.png"
        xdotool windowfocus "$window" key Escape Escape
        await_text "$window" "$out/t3-after-welcome.png" 'What should we work on' '465x103+535+355'
      else
        xdotool windowfocus "$window" key Escape
        touch "$out/t3-project-sources-unverified"
      fi
      # A profile without a provider must enter the gated scratch composer.
      xdotool windowfocus "$window" mousemove --window "$window" 828 462 click 1
      xdotool mousemove 1390 890
      assert_provider_gate "$window" "$out/t3-scratch-draft.png"
      xdotool windowfocus "$window" mousemove --window "$window" 800 424 click 1
      xdotool mousemove 1390 890
      if ! await_text "$window" "$out/t3-provider-settings.png" 'Add provider|Refresh provider status'; then
        touch "$out/t3-provider-settings-unverified"
      fi
    else
      touch "$out/t3-landing-unverified"
    fi
  elif [[ "$name" == butler && "${VERIFY_BUTLER_ONBOARDING:-0}" == 1 ]]; then
    # Coordinates refer to the fixed-size Welcome fixture captured above.
    # Check durable state and a rendered route, not only a successful X11 click.
    [[ ! -e "$runtime/state/butler/onboarding" ]]
    xdotool windowfocus "$window" mousemove --window "$window" 365 620 click 1
    xdotool mousemove 1390 890
    for ((tick=0; tick<15; tick++)); do
      [[ -f "$runtime/state/butler/onboarding" ]] && break
      kill -0 "$pid" 2>/dev/null || return 1
      sleep 1
    done
    if ! grep -qx 'butler-onboarding-complete-v1' "$runtime/state/butler/onboarding"; then
      echo "Welcome skip did not persist onboarding completion" >&2
      return 1
    fi
    await_text "$window" "$out/butler-after-welcome.png" 'What should we work on' '465x103+535+355'
    verify_scratch_draft "$window" "$out/butler-after-welcome"
  elif [[ "$name" == butler-reopened ]]; then
    verify_scratch_draft "$window" "$out/butler-reopened"
    xdotool windowfocus "$window" mousemove --window "$window" 800 424 click 1
    xdotool mousemove 1390 890
    await_text "$window" "$out/butler-provider-settings.png" 'No providers configured'
  elif [[ "$name" == butler-import ]]; then
    xdotool windowfocus "$window" mousemove --window "$window" 670 462 click 1
    xdotool mousemove 1390 890
    await_text "$window" "$out/butler-add-project.png" 'Local[[:space:]]*folder' '260x24+365+214'
    xdotool windowfocus "$window" mousemove --window "$window" 438 230 click 1
    xdotool mousemove 1390 890
    await_text "$window" "$out/butler-local-folder.png" 'Absolute[[:space:]]*folder[[:space:]]*path' '270x40+390+84'
    xdotool windowfocus "$window" mousemove --window "$window" 456 104 click 1
    xdotool type --clearmodifiers "$project_fixture"
    await_text "$window" "$out/butler-local-folder-typed.png" 'ParityFixture' '510x40+390+84'
    xdotool windowfocus "$window" mousemove --window "$window" 419 178 click 1
    xdotool mousemove 1390 890
    await_text "$window" "$out/butler-imported.png" 'ParityFixture'
    grep -Fxq "$project_fixture" "$runtime/state/butler/local-projects"
    [[ "$(git -C "$project_fixture" rev-parse --show-toplevel)" == "$project_fixture" ]]
    [[ -z "$(git -C "$project_fixture" status --porcelain)" ]]
  fi
)

await_text() {
  local window=$1 file=$2 expected=$3 crop=${4:-} tick actual
  local -a image_options ocr_options
  if [[ -n "$crop" ]]; then
    image_options=(-crop "$crop" +repage -resize 350%)
    if [[ "$crop" == '465x103+535+355' ]]; then
      ocr_options=(--psm 6)
    else
      ocr_options=(--psm 7)
    fi
  else
    image_options=(-resize 150%)
    ocr_options=(--psm 11)
  fi
  for ((tick=0; tick<45; tick++)); do
    "${capture[@]}" -window "$window" "$file"
    # OCR the real frame; window chrome alone does not imply app readiness.
    actual=$(
      "${transform[@]}" "$file" "${image_options[@]}" -colorspace gray -contrast-stretch 1%x1% png:- |
        tesseract stdin stdout "${ocr_options[@]}" 2>/dev/null
    )
    if grep -qiE "$expected" <<<"$actual"; then return 0; fi
    sleep 1
  done
  printf 'Expected text "%s" in %s; last recognized text:\n%s\n' "$expected" "$file" "$actual" >&2
  return 1
}

assert_provider_gate() {
  local window=$1 file=$2
  await_text "$window" "$file" 'Enable a provider in Settings to send a message' '600x64+295+333'
}

verify_scratch_draft() {
  local window=$1 prefix=$2
  xdotool windowfocus "$window" mousemove --window "$window" 828 462 click 1
  xdotool mousemove 1390 890
  assert_provider_gate "$window" "$prefix-scratch.png"
}

set +e
capture_one t3 "$t3/AppRun"
reference_rc=$?
capture_one butler "$butler"
butler_rc=$?
set -e
compare_frames() {
  local first=$1 second=$2 stem=$3 rc=0
  "${compare[@]}" -metric AE "$out/$first.png" "$out/$second.png" \
    "$out/$stem-diff.png" 2>"$out/$stem-differing-pixels.txt" || rc=$?
  if (( rc > 1 )); then
    cat "$out/$stem-differing-pixels.txt" >&2
    return "$rc"
  fi
  printf '%s differing pixels: %s\n' "$stem" "$(cat "$out/$stem-differing-pixels.txt")"
}

if (( reference_rc == 0 && butler_rc == 0 )); then
  compare_frames t3-startup butler-startup startup
  if [[ ! -e "$out/t3-landing-unverified" && -f "$out/t3-after-welcome.png" && -f "$out/butler-after-welcome.png" ]]; then
    compare_frames t3-after-welcome butler-after-welcome empty-workspace
    if [[ -f "$out/t3-scratch-draft.png" && -f "$out/butler-after-welcome-scratch.png" ]]; then
      compare_frames t3-scratch-draft butler-after-welcome-scratch scratch-provider-gate
    fi
  fi
fi
if (( reference_rc > 0 || butler_rc > 0 )); then
  printf 'Native capture failed (reference=%s, Butler=%s)\n' "$reference_rc" "$butler_rc" >&2
  exit 1
fi
if [[ "${VERIFY_BUTLER_ONBOARDING:-0}" == 1 ]]; then
  CAPTURE_PROFILE=butler capture_one butler-reopened "$butler"
  echo "Welcome skip and profile restart smoke test passed"
  project_fixture="$tmp/ParityFixture"
  mkdir -p "$project_fixture"
  git -C "$project_fixture" init -q
  CAPTURE_PROFILE=butler capture_one butler-import "$butler"
  CAPTURE_PROFILE=butler capture_one butler-project-reopened "$butler"
  echo "Local project import and profile restart smoke test passed"
  if [[ ! -e "$out/t3-provider-settings-unverified" && -f "$out/t3-provider-settings.png" && -f "$out/butler-provider-settings.png" ]]; then
    compare_frames t3-provider-settings butler-provider-settings provider-settings
  fi
fi
if [[ -e "$out/t3-landing-unverified" ]]; then
  echo "T3 Code post-onboarding state was not verified" >&2
  exit 1
fi
