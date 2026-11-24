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
new_project_title='Native Created Project'
new_project_slug='native-created-project'
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
  local runtime="$tmp/$profile" window="" tick dimensions ready="" window_pid window_xid=""
  local log="$out/$name.log"
  mkdir -p -m 700 "$runtime"/{config,cache,data,state,run}
  if [[ "$name" == t3-thread-find ]]; then
    # The upstream release smoke launches the pinned executable directly and
    # opens the thread by its DOM row. Use that known-good navigation only
    # for this optional fixture, then keep the actual Electron window alive
    # for the same X11 capture and OCR gates as before.
    ready="$runtime/electron.ready"
    set -- env ELECTRON_RUN_AS_NODE=1 timeout --kill-after=8s 210s "$1" \
      "$(dirname -- "${BASH_SOURCE[0]}")/diagnose-reference-find.cjs" \
      "$t3" "$out/t3-thread-find-renderer-diagnostic.json" "$ready" "$width" "$height" \
      "$out/t3-thread-find-renderer-diagnostic.png"
    # Do not upload raw helper or Chromium logs.
    log=/dev/null
  fi
  XDG_CONFIG_HOME="$runtime/config" XDG_CACHE_HOME="$runtime/cache" \
    XDG_DATA_HOME="$runtime/data" XDG_STATE_HOME="$runtime/state" \
    XDG_RUNTIME_DIR="$runtime/run" "$@" >"$log" 2>&1 &
  local pid=$!
  trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT

  # Both toolkits set _NET_WM_PID on their top-level client; ignore helper windows.
  for ((tick=0; tick<90; tick++)); do
    kill -0 "$pid" 2>/dev/null || { cat "$log" >&2; return 1; }
    window_pid="$pid"
    if [[ -n "$ready" ]]; then
      if [[ ! -s "$ready" ]]; then sleep 1; continue; fi
      read -r window_pid window_xid <"$ready" || return 1
      [[ "$window_pid" =~ ^[1-9][0-9]*$ && "$window_xid" =~ ^[1-9][0-9]*$ ]] &&
        kill -0 "$window_pid" 2>/dev/null || return 1
    fi
    while read -r id; do
      local geometry
      [[ -z "$window_xid" || "$id" == "$window_xid" ]] || continue
      geometry=$(xdotool getwindowgeometry --shell "$id" 2>/dev/null) || continue
      if grep -Eq '^WIDTH=([5-9][0-9]{2}|[0-9]{4,})$' <<<"$geometry" &&
         grep -Eq '^HEIGHT=([3-9][0-9]{2}|[0-9]{4,})$' <<<"$geometry"; then
        window=$id
        break
      fi
    done < <(xdotool search --onlyvisible --pid "$window_pid" 2>/dev/null || true)
    [[ -z "$window" ]] || break
    sleep 1
  done
  if [[ -z "$window" ]]; then
    xwininfo -root -tree >&2 || true
    cat "$log" >&2
    echo "No visible client window for $name (pid $pid)" >&2
    return 1
  fi

  if [[ -z "$ready" ]]; then
    xdotool windowmove "$window" 0 0 windowsize "$window" "$width" "$height"
  fi
  xdotool mousemove 1390 890
  sleep 3
  if [[ "$name" == t3 || "$name" == t3-new-project-form || "$name" == t3-git-url-review || "$name" == butler ]]; then
    await_text "$window" "$out/$name-startup.png" 'Start without an agent' '240x42+270+601'
  elif [[ "$name" == butler-project-reopened ]]; then
    await_text "$window" "$out/$name-startup.png" 'ParityFixture'
  elif [[ "$name" == butler-created-project-reopened ]]; then
    await_new_project_workspace "$window" "$out/$name-startup.png"
  elif [[ "$name" == butler-thread-chooser ]]; then
    await_thread_draft "$window" "$out/$name-startup.png" 'Project Beta'
  elif [[ "$name" == t3-thread-find || "$name" == butler-thread-find ]]; then
    local thread_title='Release launch check'
    if [[ "$name" == butler-thread-find ]]; then thread_title='Does the release open'; fi
    local title_crop
    title_crop=$(thread_find_row_crop "${name%-thread-find}") || return 1
    await_project_phrase "$window" "$out/$name-startup.png" "$title_crop" "$thread_title" 11 >/dev/null || return 1
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
      if ! await_text "$window" "$out/t3-provider-settings.png" 'Add provider|Refresh provider status' ||
         ! assert_disabled_cursor "$window" "$out/t3-provider-settings.png"; then
        touch "$out/t3-provider-settings-unverified"
      else
        capture_about_settings "$window" t3
      fi
    else
      touch "$out/t3-landing-unverified"
    fi
  elif [[ "$name" == t3-new-project-form ]]; then
    capture_reference_new_project_form "$window"
  elif [[ "$name" == t3-git-url-review || "$name" == butler-git-url-review ]]; then
    capture_git_url_review "$window" "${name%-git-url-review}" "$runtime"
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
    await_text "$window" "$out/butler-provider-settings.png" 'Display[[:space:]]*name'
    assert_disabled_cursor "$window" "$out/butler-provider-settings.png"
    if [[ "${CAPTURE_REFERENCE_LANDING:-0}" == 1 ]]; then
      capture_about_settings "$window" butler
    fi
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
  elif [[ "$name" == butler-create-project ]]; then
    create_native_project "$window" "$runtime/state/butler"
  elif [[ "$name" == butler-created-project-reopened ]]; then
    assert_new_project_files "$runtime/state/butler"
  elif [[ "$name" == butler-thread-chooser ]]; then
    capture_native_thread_chooser "$window" "$runtime"
  elif [[ "$name" == t3-thread-find || "$name" == butler-thread-find ]]; then
    capture_native_thread_find "$window" "${name%-thread-find}" "$thread_title" "$ready"
  fi
)

await_text() {
  local window=$1 file=$2 expected=$3 crop=${4:-} tick actual
  local -a image_options ocr_options
  if [[ -n "$crop" ]]; then
    image_options=(-crop "$crop" +repage -resize 350%)
    if [[ "$crop" == '465x103+535+355' || "$crop" == '190x85+620+265' ]]; then
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

assert_disabled_cursor() {
  local window=$1 file=$2
  # Read the editor's status, not a different provider's list label.
  await_text "$window" "$file" '^[[:space:]]*Disabled[[:space:]]*$' '190x85+620+265'
}

# Read words from only the sidebar or the page body and convert the exact
# phrase's Tesseract bounding box back to X11 client coordinates. This avoids
# hard-coding row/button positions that differ between the two apps.
settings_ocr() {
  local file=$1 crop=$2 psm=$3
  "${transform[@]}" "$file" -crop "$crop" +repage -resize 300% \
    -colorspace gray -contrast-stretch 1%x1% png:- |
    tesseract stdin stdout --psm "$psm" tsv 2>/dev/null
}

settings_phrase() {
  local file=$1 crop=$2 phrase=$3 psm=${4:-11}
  local roi_height roi_width x y
  if [[ ! "$crop" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]]; then
    return 1
  fi
  roi_width=${BASH_REMATCH[1]}; roi_height=${BASH_REMATCH[2]}
  x=${BASH_REMATCH[3]}; y=${BASH_REMATCH[4]}
  settings_ocr "$file" "$crop" "$psm" |
    awk -F '\t' -v phrase="$phrase" -v x="$x" -v y="$y" -v w="$roi_width" -v h="$roi_height" '
      BEGIN { n = split(tolower(phrase), want, " ") }
      $1 == 5 && $11 >= 20 && $12 != "" {
        key = $2 FS $3 FS $4 FS $5
        if (key != line) { line = key; matched = 0 }
        word = tolower($12)
        gsub(/^[[:punct:]]+|[[:punct:]]+$/, "", word)
        if (word == "") next
        if (word != want[matched+1]) matched = 0
        if (word != want[matched+1]) next
        if (matched == 0) { left = $7; top = $8; bottom = $8 + $10 }
        if ($8 < top) top = $8
        if ($8 + $10 > bottom) bottom = $8 + $10
        if (++matched == n) {
          cx = (left + $7 + $9) / 6; cy = (top + bottom) / 6
          if (!found && cx >= 0 && cx < w && cy >= 0 && cy < h) {
            printf "%d %d\n", x + cx, y + cy
            found = 1
          }
          matched = 0
        }
      }
      END { if (!found) exit 1 }
    '
}

wait_for_settings_view() {
  local window=$1 file=$2 title=$3 heading=$4 tick header
  # The breadcrumb and first section heading must both match in the same
  # 1280×800 frame; the About sidebar label alone is not a route check.
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    header=$(settings_phrase "$file" '410x45+288+42' "Settings $title" 7) || header=
    if [[ -n "$header" ]] && settings_phrase "$file" '932x250+294+99' "$heading" >/dev/null; then
      return 0
    fi
    sleep 0.4
  done
  return 1
}

click_settings_phrase() {
  local window=$1 probe=$2 crop=$3 phrase=$4 scroll_x=$5 scroll_y=$6
  local found cx cy tick
  for ((tick=0; tick<7; tick++)); do
    "${capture[@]}" -window "$window" "$probe" || return 1
    if found=$(settings_phrase "$probe" "$crop" "$phrase"); then
      read -r cx cy <<<"$found"
      xdotool windowfocus "$window" mousemove --window "$window" "$cx" "$cy" click 1 || return 1
      xdotool mousemove 1390 890 || return 1
      return 0
    fi
    # Stop and preserve the final diagnostic rather than guessing a click.
    (( tick < 6 )) || break
    xdotool mousemove --window "$window" "$scroll_x" "$scroll_y" click --repeat 4 5 || return 1
    xdotool mousemove 1390 890 || return 1
    sleep 0.4
  done
  return 1
}

capture_about_settings() {
  local window=$1 app=$2 probe="$tmp/$2-about-target.png"
  local about="$out/$app-about-settings.png" notices="$out/$app-open-source-licenses.png"
  local about_unverified="$out/$app-about-settings-unverified"
  local notices_unverified="$out/$app-open-source-licenses-unverified"
  local step='find About in the Settings sidebar'
  touch "$about_unverified" "$notices_unverified"
  if click_settings_phrase "$window" "$probe" '218x704+52+48' 'About' 180 700; then
    step='verify the About breadcrumb and App section'
    if wait_for_settings_view "$window" "$about" 'About' 'App'; then
      rm -f -- "$about_unverified"
      printf '%s: verified the native About route\n' "$app"
      step='find the View licenses button on the About page'
      # The full-body sparse OCR dropped Butler's visible label in
      # run 45363944626. These button crops were verified on both real frames.
      local licenses_crop='140x42+1042+637'
      if [[ "$app" == butler ]]; then licenses_crop='140x42+1042+302'; fi
      if click_settings_phrase "$window" "$probe" "$licenses_crop" 'View licenses' 1120 700; then
        step='verify the Open source licenses breadcrumb and Third-party notices heading'
        if wait_for_settings_view "$window" "$notices" 'Open source licenses' 'Third-party notices'; then
          rm -f -- "$notices_unverified"
          printf '%s: verified the native Third-party notices route\n' "$app"
          return 0
        fi
      fi
    fi
  fi
  if [[ -f "$probe" ]]; then
    cp -- "$probe" "$out/$app-about-target-unverified.png"
  fi
  printf '%s: optional About capture incomplete (could not %s); see unverified markers\n' \
    "$app" "$step" >&2
  return 0
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

await_project_phrase() {
  local window=$1 file=$2 crop=$3 phrase=$4 psm=$5 coords tick
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    if coords=$(settings_phrase "$file" "$crop" "$phrase" "$psm"); then
      printf '%s\n' "$coords"
      return 0
    fi
    sleep 0.4
  done
  printf 'Could not find "%s" in %s (crop %s)\n' "$phrase" "$file" "$crop" >&2
  return 1
}

click_project_control() {
  local window=$1; shift
  local coords x y
  coords=$(await_project_phrase "$window" "$@") || return 1
  read -r x y <<<"$coords"
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool mousemove 1390 890
}

await_new_project_workspace() {
  local window=$1 file=$2 tick
  # Match the selected project in the workspace composer, not text in
  # the picker or the breadcrumb's responsive layout; confirm the gate too.
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    if settings_phrase "$file" '380x33+485+290' "$new_project_title" 7 >/dev/null &&
       settings_phrase "$file" '600x64+295+333' \
         'Enable a provider in Settings to send a message' 7 >/dev/null; then
      return 0
    fi
    sleep 0.4
  done
  printf 'Could not verify the project workspace title and provider gate in %s\n' "$file" >&2
  return 1
}

assert_new_project_files() {
  local profile=$1 root canonical tracked registry
  root="$profile/projects/$new_project_slug"
  canonical=$(realpath -e -- "$root") || return 1
  [[ "$canonical" == "$root" && -d "$root/.git" ]] || return 1
  [[ "$(git -C "$root" rev-parse --show-toplevel)" == "$canonical" ]] || return 1
  grep -Fxq -- "# $new_project_title" "$root/README.md" || return 1
  [[ -s "$root/assets/icon.svg" ]] || return 1
  # ls-files visits the index both after commit and when no Git identity is
  # configured and Butler leaves the two files staged instead.
  tracked=$(git -C "$root" ls-files) || return 1
  [[ "$tracked" == $'README.md\nassets/icon.svg' ]] || return 1
  registry=$(cat -- "$profile/local-projects") || return 1
  [[ "$registry" == "$(printf '%s\ntitle\t"%s"' "$canonical" "$new_project_title")" ]]
}

create_native_project() {
  local window=$1 profile=$2
  [[ ! -e "$profile/local-projects" && ! -e "$profile/projects/$new_project_slug" ]] || return 1
  # The landing button, source palette and inherited input row bounds are
  # from the existing 1280×800 import captures. A crop only limits OCR;
  # the script never clicks without a recognized, exact control label.
  click_project_control "$window" "$out/butler-create-project-landing.png" \
    '160x40+604+442' 'Add project' 7 || return 1
  click_project_control "$window" "$out/butler-create-project-sources.png" \
    '574x240+353+81' 'New project' 11 || return 1
  click_project_control "$window" "$out/butler-create-project-name.png" \
    '510x40+390+84' 'Project name' 7 || return 1
  xdotool type --clearmodifiers "$new_project_title" || return 1
  await_project_phrase "$window" "$out/butler-create-project-named.png" \
    '510x40+390+84' "$new_project_title" 7 >/dev/null || return 1
  click_project_control "$window" "$out/butler-create-project-submit.png" \
    '114x36+807+87' 'Create' 7 || return 1
  await_new_project_workspace "$window" "$out/butler-created-project.png" || return 1
  assert_new_project_files "$profile" || {
    echo 'Created workspace rendered, but its local Git scaffold or project registry did not match' >&2
    return 1
  }
}

verify_native_new_project() {
  local profile='butler-new-project' location rc
  location="$tmp/$profile/state/butler"
  touch "$out/butler-create-project-unverified" "$out/butler-created-project-reopened-unverified"
  # Onboarding has its own native check. Seed only its documented marker
  # here, keeping this profile isolated from the existing import and About fixtures.
  mkdir -p "$location"
  printf '%s\n' 'butler-onboarding-complete-v1' >"$location/onboarding"
  set +e
  CAPTURE_PROFILE="$profile" capture_one butler-create-project "$butler"
  rc=$?
  set -e
  if (( rc != 0 )); then
    echo 'Requested native New project smoke failed; see butler-create-project-unverified' >&2
    return 1
  fi
  rm -f -- "$out/butler-create-project-unverified"
  set +e
  CAPTURE_PROFILE="$profile" capture_one butler-created-project-reopened "$butler"
  rc=$?
  set -e
  if (( rc != 0 )); then
    echo 'Requested native New project reopen failed; see butler-created-project-reopened-unverified' >&2
    return 1
  fi
  rm -f -- "$out/butler-created-project-reopened-unverified"
  echo 'Native New project creation, local Git scaffold, and title after restart verified'
}

await_reference_project_form() {
  local window=$1 file=$2 input=$3 destination=$4 tick coords
  # Project name is unique to the pinned New project form. Confirm the
  # destination preview in the same frame so a similarly named row is not
  # mistaken for a focusable input or a completed name change.
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    coords=$(settings_phrase "$file" '510x40+390+84' "$input" 7) || coords=
    if [[ -n "$coords" ]] && settings_phrase "$file" '574x190+353+125' "$destination" >/dev/null; then
      printf '%s\n' "$coords"
      return 0
    fi
    sleep 0.4
  done
  printf 'Could not verify the T3 New project form (%s and %s) in %s\n' "$input" "$destination" "$file" >&2
  return 1
}

capture_reference_new_project_form() {
  local window=$1 coords x y
  # Only open the local form in this fresh T3 profile. Never activate Create,
  # send a prompt, or open a provider/remote repository option.
  click_project_control "$window" "$out/t3-create-project-welcome.png" \
    '240x42+270+601' 'Start without an agent' 7 || return 1
  click_project_control "$window" "$out/t3-create-project-landing.png" \
    '160x40+604+442' 'Add project' 7 || return 1
  click_project_control "$window" "$out/t3-create-project-sources.png" \
    '574x240+353+81' 'New project' 11 || return 1
  [[ -n "${T3CODE_HOME:-}" && ! -e "$T3CODE_HOME/projects/$new_project_slug" ]] || return 1
  coords=$(await_reference_project_form "$window" "$out/t3-create-project-name.png" \
    'Project name' 'Goes in') || return 1
  rm -f -- "$out/t3-create-project-name-unverified"
  read -r x y <<<"$coords"
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool type --clearmodifiers "$new_project_title" || return 1
  xdotool mousemove 1390 890 || return 1
  await_reference_project_form "$window" "$out/t3-create-project-named.png" \
    "$new_project_title" 'Creates' >/dev/null || return 1
  if [[ -e "$T3CODE_HOME/projects/$new_project_slug" ]]; then
    echo 'T3 New project was unexpectedly written; do not use the comparison frames' >&2
    touch "$out/t3-create-project-name-unverified"
    return 1
  fi
  rm -f -- "$out/t3-create-project-named-unverified"
}

verify_reference_new_project_form() {
  local profile='t3-new-project-form' capture_rc state
  local notes="$out/new-project-form-capture-notes.txt"
  touch "$out/t3-create-project-name-unverified" "$out/t3-create-project-named-unverified"
  printf '%s\n' \
    'Fresh native X11 local New project forms, empty and with the same typed title: Native Created Project.' \
    'The reference form was not submitted. The Butler creation/restart smoke is separate.' \
    'The two apps use different temporary profile and project roots; path previews and raw diffs are diagnostic, not parity claims.' \
    'See provenance.txt for exact builds; any *-unverified marker means the associated frame or pair is diagnostic only.' \
    >"$notes"
  set +e
  T3CODE_HOME="$tmp/$profile/home" CAPTURE_PROFILE="$profile" capture_one "$profile" "$t3/AppRun"
  capture_rc=$?
  set -e
  if (( capture_rc != 0 )); then
    echo 'Optional T3 New project form capture incomplete; see t3-create-project-*-unverified markers' \
      | tee -a "$notes" >&2
  fi
  for state in name named; do
    if [[ "${VERIFY_BUTLER_NEW_PROJECT:-0}" == 1 && ! -e "$out/butler-create-project-unverified" &&
          ! -e "$out/t3-create-project-$state-unverified" &&
          -f "$out/t3-create-project-$state.png" && -f "$out/butler-create-project-$state.png" ]] &&
       compare_frames "t3-create-project-$state" "butler-create-project-$state" "new-project-form-$state"; then
      rm -f -- "$out/new-project-form-$state-pair-unverified"
      printf 'New project form %s: verified route pair (raw diagnostic diff)\n' "$state" | tee -a "$notes"
    else
      touch "$out/new-project-form-$state-pair-unverified"
      printf 'New project form %s: optional pair not verified\n' "$state" | tee -a "$notes" >&2
    fi
  done
}

# Compare one recognized line literally. Tesseract sometimes inserts spaces
# around URL punctuation, but case, slashes, the tilde and .git must all match.
project_literal() {
  local file=$1 crop=$2 expected=$3 psm=$4 recognized
  recognized=$(
    "${transform[@]}" "$file" -crop "$crop" +repage -resize 400% \
      -colorspace gray -contrast-stretch 1%x1% png:- |
      tesseract stdin stdout --psm "$psm" 2>/dev/null
  ) || return 1
  sed 's/[[:space:]]//g' <<<"$recognized" | grep -Fxq -- "$expected"
}

git_destination_frame() {
  local app=$1 frame=$2 destination=$3 remote=$4
  # Measured on both native destination frames, most recently run 45374623866. The
  # wide palette crop confused single-line OCR with borders, and its left
  # edge clipped the Repository heading. Read each row on its own instead.
  local remote_crop='345x22+390+177' browse_crop='180x21+363+204' action='Create&Clone'
  if [[ "$app" == t3 ]]; then
    remote_crop='345x22+390+189'
    browse_crop='180x21+363+224'
    # Both apps show Create & Clone for the vacant fixture. At this step
    # pinned T3 also shows an Enter shortcut badge; Butler does not.
    action='Create&CloneEnter'
  fi
  project_literal "$frame" '132x26+390+92' "$destination" 7 &&
    project_literal "$frame" '153x33+768+89' "$action" 7 &&
    project_literal "$frame" '120x30+363+131' 'Repository' 7 &&
    project_literal "$frame" "$remote_crop" "$remote" 7 &&
    project_literal "$frame" "$browse_crop" 'Selectwheretoclone' 7
}

capture_git_url_review() {
  local window=$1 app=$2 runtime=$3 tick coords x y
  local url_input='ButlerFixture/CaseRepo'
  local remote='https://github.com/ButlerFixture/CaseRepo.git'
  local destination='~/CaseRepo'
  local prefix="$out/$app-git-url-review"
  # Both existing New project native frames use this input row. Limit clicks
  # to the expected controls and their respective halves of the palette.
  local input_crop='395x40+389+84' submit_crop='134x42+791+83'
  if [[ "$app" == t3 ]]; then
    click_project_control "$window" "$prefix-welcome.png" \
      '240x42+270+601' 'Start without an agent' 7 || return 1
  fi
  click_project_control "$window" "$prefix-landing.png" \
    '160x40+604+442' 'Add project' 7 || return 1
  # The actual T3 and Butler Sources frames include a similarly described
  # GitHub repository row. Match the plain Git URL control, not its neighbor.
  click_project_control "$window" "$prefix-sources.png" \
    '574x380+353+81' 'Git URL' 11 || return 1
  click_project_control "$window" "$prefix-empty.png" \
    "$input_crop" 'Enter Git clone URL' 7 || return 1
  xdotool type --clearmodifiers "$url_input" || return 1
  xdotool mousemove 1390 890 || return 1

  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$prefix-typed.png" || return 1
    coords=$(settings_phrase "$prefix-typed.png" "$submit_crop" 'Continue' 7) || coords=
    if [[ -n "$coords" ]] && project_literal "$prefix-typed.png" "$input_crop" "$url_input" 7; then
      break
    fi
    sleep 0.4
  done
  if (( tick == 15 )); then
    printf '%s: did not verify the exact shorthand and Continue in the URL entry step\n' "$app" >&2
    return 1
  fi
  rm -f -- "$prefix-typed-unverified"

  # Pinned T3 CommandPalette.tsx:2191-2232 normalizes the plain URL and
  # switches to confirm locally; provider lookup is a different branch.
  # Its clone dispatch is only in the subsequent confirm step (:2278-2337).
  # This is the ONLY click on the action accessory, after OCR says Continue.
  read -r x y <<<"$coords"
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool mousemove 1390 890 || return 1

  # No more clicks or keys. Confirm the exact destination and normalized
  # remote from the same image as the Repository/Select where to clone UI.
  for ((tick=0; tick<15; tick++)); do
    local frame="$prefix-destination.png"
    "${capture[@]}" -window "$window" "$frame" || return 1
    if git_destination_frame "$app" "$frame" "$destination" "$remote"; then
      break
    fi
    sleep 0.4
  done
  if (( tick == 15 )); then
    printf '%s: did not verify %s and %s in the destination step\n' "$app" "$remote" "$destination" >&2
    return 1
  fi
  if [[ -e "$HOME/CaseRepo" || -L "$HOME/CaseRepo" ||
        -e "$runtime/state/butler/local-projects" ]] ||
     compgen -G "$HOME/.butler-clone-*" >/dev/null; then
    echo "$app: unexpected clone filesystem state; both frames are diagnostic only" >&2
    touch "$prefix-typed-unverified"
    return 1
  fi
  rm -f -- "$prefix-destination-unverified"
}

verify_git_url_review_pairs() {
  local app name runtime binary rc state
  local notes="$out/git-url-review-capture-notes.txt"
  printf '%s\n' \
    'Native Git URL review from fresh, separate X11 test profiles: ButlerFixture/CaseRepo.' \
    'Expected normalized URL: https://github.com/ButlerFixture/CaseRepo.git; displayed draft destination: ~/CaseRepo.' \
    'Continue was the last click. Clone was not clicked and no Enter was sent. T3 lists directories from its local test home.' \
    'These frames verify local review, not clone execution or transport. See provenance.txt; unverified frames are diagnostic only.' \
    >"$notes"
  for app in t3 butler; do
    name="$app-git-url-review"
    runtime="$tmp/$name"
    mkdir -p -m 700 "$runtime/home/FixtureFolder" "$runtime/state/butler"
    touch "$out/$name-typed-unverified" "$out/$name-destination-unverified"
    binary="$t3/AppRun"
    if [[ "$app" == butler ]]; then
      binary="$butler"
      printf '%s\n' 'butler-onboarding-complete-v1' >"$runtime/state/butler/onboarding"
    fi
    set +e
    HOME="$runtime/home" T3CODE_HOME="$runtime/t3-code" \
      GIT_ALLOW_PROTOCOL= GIT_TERMINAL_PROMPT=0 capture_one "$name" "$binary"
    rc=$?
    set -e
    if (( rc != 0 )); then
      printf '%s: optional Git URL review incomplete; see unverified markers\n' "$app" | tee -a "$notes" >&2
    fi
  done
  for state in typed destination; do
    if [[ ! -e "$out/t3-git-url-review-$state-unverified" &&
          ! -e "$out/butler-git-url-review-$state-unverified" ]] &&
       compare_frames "t3-git-url-review-$state" "butler-git-url-review-$state" "git-url-review-$state"; then
      rm -f -- "$out/git-url-review-$state-pair-unverified"
      printf 'Git URL %s: verified route pair (raw diagnostic diff)\n' "$state" | tee -a "$notes"
    else
      touch "$out/git-url-review-$state-pair-unverified"
      printf 'Git URL %s: optional pair not verified\n' "$state" | tee -a "$notes" >&2
    fi
  done
}

await_thread_draft() {
  local window=$1 file=$2 title=$3 tick
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    # Assert the selected workspace and actual blank draft content. The
    # responsive breadcrumb gets its own geometry regression.
    if settings_phrase "$file" '380x33+485+290' "$title" 7 >/dev/null &&
       settings_phrase "$file" '480x58+393+246' 'What should we work on' 7 >/dev/null &&
       settings_phrase "$file" '600x64+295+333' \
         'Enable a provider in Settings to send a message' 7 >/dev/null; then
      return 0
    fi
    sleep 0.4
  done
  printf 'Could not verify the %s fresh draft and provider gate in %s\n' "$title" "$file" >&2
  return 1
}

thread_chooser_registry() {
  local runtime=$1 first=$2 second=$3 title
  for title in "$first" "$second"; do
    printf '%s/fixtures/draft-%s-fixture\ntitle\t"Project %s"\n' "$runtime" "${title,,}" "$title"
  done
}

thread_chooser_snapshot() (
  cd -- "$1/fixtures" || return 1
  # Directories must not get new children or metadata changes. Ctime/mtime
  # and the sentinel hashes catch writes without treating ordinary reads as writes.
  find . -printf '%P\t%y\t%m\t%s\t%T@\t%C@\n' | LC_ALL=C sort
  sha256sum -- draft-alpha-fixture/fixture.txt draft-beta-fixture/fixture.txt
)

capture_native_thread_chooser() {
  local window=$1 runtime tick coords x y frame
  runtime=$(realpath -e -- "$2") || return 1
  local profile="$runtime/state/butler" prefix="$out/butler-thread-chooser"
  local input_crop='395x40+389+84' rows_crop='500x102+383+154'
  local placeholder='New thread in' alpha='Project Alpha' beta='Project Beta'
  # New thread is the rightmost glyph-only control in the 255px sidebar.
  # empty_workspace.rs uses a 12px right spacer, a 28px button and a
  # 48px Search row below the 40px title. The previous real project-open
  # frame shows the same gutter. Gate this source-derived click on Search
  # in the active project draft, and require the chooser after clicking.
  settings_phrase "$prefix-startup.png" '164x40+15+44' 'Search' 7 >/dev/null || {
    echo 'Could not confirm the active sidebar Search row before the New thread glyph click' >&2
    return 1
  }
  xdotool windowfocus "$window" mousemove --window "$window" 229 64 click 1 || return 1
  xdotool mousemove 1390 890 || return 1
  frame="$prefix-choices.png"
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$frame" || return 1
    coords=$(settings_phrase "$frame" "$input_crop" "$placeholder" 7) || coords=
    if [[ -n "$coords" ]] &&
       settings_phrase "$frame" '160x27+363+129' 'Projects' 7 >/dev/null &&
       settings_phrase "$frame" "$rows_crop" "$alpha" 11 >/dev/null &&
       settings_phrase "$frame" "$rows_crop" "$beta" 11 >/dev/null; then
      break
    fi
    sleep 0.4
  done
  if (( tick == 15 )); then
    echo 'Could not verify the New thread in… chooser and both fixtures' >&2
    return 1
  fi

  read -r x y <<<"$coords"
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool type --clearmodifiers 'ALPHA' || return 1
  xdotool mousemove 1390 890 || return 1
  frame="$prefix-filtered.png"
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$frame" || return 1
    coords=$(settings_phrase "$frame" "$rows_crop" "$alpha" 11) || coords=
    if [[ -n "$coords" ]] && project_literal "$frame" "$input_crop" 'ALPHA' 7 &&
       ! settings_phrase "$frame" "$rows_crop" "$beta" 11 >/dev/null; then
      break
    fi
    sleep 0.4
  done
  if (( tick == 15 )); then
    echo 'Could not verify the case-insensitive ALPHA filter and exact visible row' >&2
    return 1
  fi
  [[ "$(cat -- "$profile/local-projects")" == "$(thread_chooser_registry "$runtime" Beta Alpha)" ]] || {
    echo 'The saved project order or titles unexpectedly changed before selecting a result' >&2
    return 1
  }

  # The only click after filtering is the verified project’s OCR hitbox.
  read -r x y <<<"$coords"
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool mousemove 1390 890 || return 1
  await_thread_draft "$window" "$prefix-selected.png" "$alpha" || return 1
  [[ "$(cat -- "$profile/local-projects")" == "$(thread_chooser_registry "$runtime" Alpha Beta)" ]] &&
    [[ ! -e "$profile/threads" && ! -e "$profile/scratch" ]] || {
      echo 'Draft rendered, but saved projects (order/titles) or no-send state did not match' >&2
      return 1
    }
}

verify_native_thread_chooser() {
  local name='butler-thread-chooser' runtime profile before after rc
  local notes="$out/butler-thread-chooser-notes.txt" marker="$out/butler-thread-chooser-unverified"
  runtime=$(realpath -m "$tmp/$name") || return 1
  profile="$runtime/state/butler"
  touch "$marker"
  printf '%s\n' \
    'Native X11 New thread chooser, with two untouched local fixtures seeded in a fresh profile.' \
    'Starts on Project Beta; both named rows must render; ALPHA must filter to Project Alpha without switching until clicked.' \
    'The selected project must open an unsent draft with its provider gate, become most recent, and preserve both display titles.' \
    'No provider, message or remote project action is used. If the unverified marker exists, all frames are diagnostic only.' \
    >"$notes"
  mkdir -p -m 700 "$profile" "$runtime/fixtures"/{draft-alpha-fixture,draft-beta-fixture}
  printf '%s\n' 'butler-onboarding-complete-v1' >"$profile/onboarding"
  thread_chooser_registry "$runtime" Beta Alpha >"$profile/local-projects"
  printf '%s\n' 'Alpha read-only fixture' >"$runtime/fixtures/draft-alpha-fixture/fixture.txt"
  printf '%s\n' 'Beta read-only fixture' >"$runtime/fixtures/draft-beta-fixture/fixture.txt"
  before=$(thread_chooser_snapshot "$runtime") || return 1
  set +e
  CAPTURE_PROFILE="$name" capture_one "$name" "$butler"
  rc=$?
  set -e
  after=$(thread_chooser_snapshot "$runtime") || return 1
  if [[ "$before" != "$after" ]]; then
    echo 'The chooser changed a fixture tree; native verification failed' | tee -a "$notes" >&2
    return 1
  fi
  if (( rc != 0 )); then
    echo 'Requested native thread chooser smoke failed; see the unverified marker' | tee -a "$notes" >&2
    return 1
  fi
  rm -f -- "$marker"
  echo 'Verified native chooser choices, filter, fresh draft, saved project order/titles, and unchanged fixture trees' | tee -a "$notes"
}

thread_find_row_crop() {
  if [[ "$1" == butler ]]; then
    # Verified on the real 5c2504d fixture frame. The generic tall
    # sidebar crop misses this one thin row when no other history exists.
    # Keep room for the taller project caption introduced at 93eaf3a.
    printf '%s\n' '220x82+8+148'
  else
    printf '%s\n' '246x650+6+75'
  fi
}

thread_find_transcript() {
  local app=$1 file=$2 crop='940x718+281+43' psm=11
  if [[ "$app" == butler ]]; then
    # The saved user bubble is now flush right in the lane, and the reply
    # stays left. Verify separate message regions, both outside the Find bar.
    settings_phrase "$file" '440x86+540+43' 'Does the release open' 11 >/dev/null &&
      settings_phrase "$file" '425x85+291+89' 'The release opens and shows this thread' 11 >/dev/null
    return
  fi
  settings_phrase "$file" "$crop" 'Does the release open' "$psm" >/dev/null &&
    settings_phrase "$file" "$crop" 'The release opens and shows this thread' "$psm" >/dev/null
}

await_thread_find_count() {
  local app=$1 window=$2 file=$3 query=$4 count=$5 anchor_y=$6 tick coords unused y
  local input="135x29+995+$((anchor_y - 14))"
  for ((tick=0; tick<15; tick++)); do
    "${capture[@]}" -window "$window" "$file" || return 1
    coords=$(settings_phrase "$file" "$input" "$query" 7) || coords=
    if [[ -n "$coords" ]]; then
      read -r unused y <<<"$coords"
      if (( y >= anchor_y-5 && y <= anchor_y+5 )) &&
         project_literal "$file" "$input" "$query" 7 &&
         thread_find_count_label "$file" "$count" "$y" &&
         thread_find_transcript "$app" "$file"; then
        return 0
      fi
    fi
    sleep 0.4
  done
  printf 'Could not verify Find input %s, count %s, and both transcript messages in %s\n' "$query" "$count" "$file" >&2
  return 1
}

thread_find_count_label() {
  local file=$1 count=$2 center_y=$3 top
  top=$((center_y - 12))
  # The input and count share a baseline. Isolate the label before the
  # glyph-only arrow buttons; recognize the slash and both digits literally.
  project_literal "$file" "49x25+1138+$top" "$count" 7 ||
    settings_ocr "$file" "87x25+1112+$top" 11 |
      awk -F '\t' -v want="$count" '$1 == 5 && $11 >= 20 && $12 == want {found=1} END {exit !found}'
}

capture_native_thread_find() {
  local window=$1 app=$2 title=$3 ready=$4 coords x y anchor_y input title_crop deadline
  local prefix="$out/$app-thread-find" bar='278x92+988+37'
  if [[ "$app" == butler ]]; then
    title_crop=$(thread_find_row_crop "$app") || return 1
    click_project_control "$window" "$prefix-thread-row.png" "$title_crop" "$title" 11 || return 1
  fi
  deadline=$((SECONDS + 60))
  while (( SECONDS < deadline )); do
    "${capture[@]}" -window "$window" "$prefix-transcript.png" || return 1
    if thread_find_transcript "$app" "$prefix-transcript.png"; then break; fi
    sleep 0.5
  done
  if ! thread_find_transcript "$app" "$prefix-transcript.png"; then
    echo "$app: could not verify both saved messages in the opened transcript" >&2
    if [[ "$app" == t3 && -n "$ready" ]]; then
      # For diagnosis only, request same-size Chromium pixels from the exact
      # BrowserWindow. This frame can't pass or replace native OCR.
      touch "$ready.diagnostic" || return 1
      deadline=$((SECONDS + 8))
      while [[ ! -f "$ready.diagnosed" ]] && (( SECONDS < deadline )); do sleep 0.25; done
    fi
    return 1
  fi

  # Incular button clicks don't take keyboard focus. Normal Tab moves focus
  # inside the selected workspace so Butler's scoped Find shortcut can run.
  if [[ "$app" == butler ]]; then xdotool windowfocus "$window" key Tab || return 1; fi
  xdotool windowfocus "$window" key ctrl+f || return 1
  coords=$(await_project_phrase "$window" "$prefix-open.png" "$bar" 'Find in thread' 11) || return 1
  # Only type after the Find placeholder is OCR-verified in the pane's
  # top-right bar. Never press Return or click a send/provider control.
  read -r x y <<<"$coords"
  anchor_y=$y
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 || return 1
  xdotool type --clearmodifiers 'release' || return 1
  xdotool mousemove 1390 890 || return 1
  await_thread_find_count "$app" "$window" "$prefix-match.png" 'release' '1/2' "$anchor_y" || return 1

  # Both pinned bars end 12 px from the pane edge, with 1 px border,
  # 4 px right padding and three 24 px buttons separated by 2 px.
  # Only after OCR established Find and 1/2, click the middle (Next) slot,
  # centered at x=1225 in the 1280 px client. The count must actually advance.
  local next_x=$((width - 12 - 1 - 4 - 12 - 26))
  if xdotool windowfocus "$window" mousemove --window "$window" "$next_x" "$anchor_y" click 1 &&
     xdotool mousemove 1390 890 &&
     await_thread_find_count "$app" "$window" "$prefix-next.png" 'release' '2/2' "$anchor_y"; then
    rm -f -- "$prefix-next-unverified" || return 1
  else
    echo "$app: optional Find Next diagnostic not verified" >&2
  fi

  # Read the live input again, not the earlier 1/2 frame. A failed Next
  # count does not prevent verifying no-match when the input is still there.
  input="135x29+995+$((anchor_y - 14))"
  coords=$(await_project_phrase "$window" "$prefix-before-no-match.png" "$input" 'release' 7) || return 1
  project_literal "$prefix-before-no-match.png" "$input" 'release' 7 || return 1
  read -r x y <<<"$coords"
  (( y >= anchor_y-5 && y <= anchor_y+5 )) || return 1
  xdotool windowfocus "$window" mousemove --window "$window" "$x" "$y" click 1 key ctrl+a || return 1
  xdotool type --clearmodifiers 'absent' || return 1
  xdotool mousemove 1390 890 || return 1
  await_thread_find_count "$app" "$window" "$prefix-no-match.png" 'absent' '0/0' "$anchor_y"
}

seed_butler_thread_find() {
  local runtime=$1 profile=$1/state/butler project=$1/project
  local thread='a0142a87-8050-43fc-9d39-038947dff1aa'
  mkdir -p -m 700 "$profile/threads" "$project" || return 1
  printf '%s\n' 'butler-onboarding-complete-v1' >"$profile/onboarding" || return 1
  printf '%s\ntitle\t"Launch Check"\n' "$project" >"$profile/local-projects" || return 1
  python3 - "$profile/threads/$thread.json" "$thread" "$project" <<'PY'
import json
import pathlib
import sys

output, thread, project = sys.argv[1:]
pathlib.Path(output).write_text(json.dumps([2, {
    "id": thread,
    "workspace": {"Local": project},
    "prompt": "Does the release open?",
    "reply": "The release opens and shows this thread.",
    "follow_ups": [],
}]), encoding="utf-8")
PY
}

butler_thread_find_snapshot() (
  cd -- "$1" || return 1
  sha256sum -- state/butler/local-projects state/butler/threads/*.json || return 1
  find project state/butler/threads -printf '%p\t%y\t%m\t%s\t%T@\t%C@\n' | LC_ALL=C sort
)

t3_find_diagnostic() {
  local home=$1 summary="$out/t3-thread-find-backend-diagnostic.txt"
  local log="$home/userdata/logs/server-child.log"
  python3 - "$home/userdata/statev2.sqlite" >"$summary" <<'PY'
import pathlib
import sqlite3
import sys

path = pathlib.Path(sys.argv[1])
thread = "launch-check-thread"
try:
    with sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True) as db:
        for label, query in (
            ("thread rows", "select count(*) from orchestration_v2_projection_threads where thread_id=?"),
            ("message rows", "select count(*) from orchestration_v2_projection_messages where thread_id=?"),
            ("message turn items", "select count(*) from orchestration_v2_projection_turn_items where thread_id=? and type in ('user_message', 'assistant_message')"),
        ):
            try:
                print(f"{label}: {db.execute(query, (thread,)).fetchone()[0]}")
            except sqlite3.Error as error:
                print(f"{label}: {type(error).__name__}")
except sqlite3.Error as error:
    print(f"Could not inspect disposable reference fixture: {type(error).__name__}")
PY
  if [[ -f "$log" ]]; then
    # Count evidence, not raw log lines: loopback authorization values or
    # runtime configuration may appear in server logs even with a fresh home.
    local pattern n
    for pattern in 'timeline' 'SqliteError|SQLITE_' 'SchemaError|ParseError' 'launch-check-thread'; do
      n=$(grep -Eic "$pattern" "$log") || true
      printf 'Backend log matching /%s/: %s\n' "$pattern" "${n:-0}" >>"$summary"
    done
  else
    printf '%s\n' 'Packaged backend did not emit a server-child.log.' >>"$summary"
  fi
}

verify_native_thread_find() {
  local source=$1 app name runtime binary before after state rc
  local fixture_sha='e65f15f89a847a264cf4366cd7bcdc31cef1d5fbfd07707ed497cc0f045357b5'
  local notes="$out/thread-find-capture-notes.txt"
  touch "$out/t3-thread-find-unverified" "$out/butler-thread-find-unverified" \
    "$out/t3-thread-find-next-unverified" "$out/butler-thread-find-next-unverified" \
    "$out/thread-find-match-pair-unverified" "$out/thread-find-next-pair-unverified" \
    "$out/thread-find-no-match-pair-unverified" || return 1
  printf '%s\n' \
    'Paired native X11 Find diagnostic using two identical synthetic saved messages; sidebar titles differ by app.' \
    'T3 Code v0.1.11 (59651c1761f6c63e9b6056839067066968e6a406), official scripts/fixtures/desktop-launch/statev2.sqlite (MIT, T3 Tools Inc.).' \
    "Reference fixture SHA256: $fixture_sha" \
    'See provenance.txt for Butler commit. No provider setup, prompt submission, or Return key is used.' \
    'T3 opens the seeded thread with its bundled Playwright; both apps use real X11 pixels for verification and capture.' \
    'Each verified frame shows the existing transcript and the matching query/count. Raw diffs are diagnostic, not pixel parity.' \
    >"$notes" || return 1
  for app in t3 butler; do
    name="$app-thread-find"
    runtime=$(realpath -m -- "$tmp/$name") || return 1
    mkdir -p -m 700 "$runtime/home" || return 1
    if [[ "$app" == t3 ]]; then
      # Upstream release smoke uses a disposable copy, because startup runs
      # migrations. Its persisted project path must not target an existing user folder.
      if [[ ! -f "$source" || -e /tmp/t3-launch-check-workspace || -L /tmp/t3-launch-check-workspace ]] ||
         ! mkdir -p -m 700 "$runtime/t3-code/userdata" ||
         ! cp -- "$source" "$runtime/t3-code/userdata/statev2.sqlite" ||
         ! printf '%s  %s\n' "$fixture_sha" "$runtime/t3-code/userdata/statev2.sqlite" | sha256sum --check --status; then
        echo 'T3 Find fixture missing, checksum mismatched, or fixture workspace already exists; T3 not launched.' | tee -a "$notes" >&2
        continue
      fi
      binary="$t3/t3code"
    else
      binary="$butler"
      seed_butler_thread_find "$runtime" || continue
      before=$(butler_thread_find_snapshot "$runtime") || continue
    fi
    HOME="$runtime/home" T3CODE_HOME="$runtime/t3-code" T3CODE_TELEMETRY_ENABLED=false \
      T3CODE_DISABLE_AUTO_UPDATE=true GIT_ALLOW_PROTOCOL= GIT_TERMINAL_PROMPT=0 \
      CAPTURE_PROFILE="$name" capture_one "$name" "$binary"
    rc=$?
    if [[ "$app" == t3 ]]; then t3_find_diagnostic "$runtime/t3-code"; fi
    if (( rc != 0 )); then
      printf '%s: native Find diagnostic incomplete; see its unverified marker\n' "$app" | tee -a "$notes" >&2
      continue
    fi
    if [[ "$app" == butler ]]; then
      after=$(butler_thread_find_snapshot "$runtime") || continue
      if [[ "$before" != "$after" || -e "$runtime/state/butler/scratch" ]]; then
        echo 'Butler Find fixture changed unexpectedly; do not use its frames as verified' | tee -a "$notes" >&2
        touch "$out/$name-next-unverified" || return 1
        continue
      fi
    fi
    rm -f -- "$out/$name-unverified" || return 1
    printf '%s: verified transcript, release 1/2, and absent 0/0\n' "$app" | tee -a "$notes"
    if [[ ! -e "$out/$name-next-unverified" ]]; then
      printf '%s: verified the native Next button advanced to 2/2\n' "$app" | tee -a "$notes"
    fi
  done
  for state in match next no-match; do
    if [[ ! -e "$out/t3-thread-find-unverified" && ! -e "$out/butler-thread-find-unverified" ]] &&
       { [[ "$state" != next ]] ||
         [[ ! -e "$out/t3-thread-find-next-unverified" && ! -e "$out/butler-thread-find-next-unverified" ]]; } &&
       compare_frames "t3-thread-find-$state" "butler-thread-find-$state" "thread-find-$state"; then
      rm -f -- "$out/thread-find-$state-pair-unverified" || return 1
      printf 'Thread Find %s: verified route pair (raw diagnostic diff)\n' "$state" | tee -a "$notes"
    else
      printf 'Thread Find %s: optional pair not verified\n' "$state" | tee -a "$notes" >&2
    fi
  done
}

set +e
capture_one t3 "$t3/AppRun"
reference_rc=$?
capture_one butler "$butler"
butler_rc=$?
set -e
compare_frames() {
  local first=$1 second=$2 stem=$3 rc=0
  local frame dimensions
  for frame in "$first" "$second"; do
    dimensions=$("${identify[@]}" -format '%wx%h' "$out/$frame.png") || return 1
    if [[ "$dimensions" != "${width}x${height}" ]]; then
      printf '%s frame is %s, expected %sx%s\n' "$frame" "$dimensions" "$width" "$height" >&2
      return 1
    fi
  done
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
  if [[ "${CAPTURE_REFERENCE_LANDING:-0}" == 1 ]]; then
    if [[ -e "$out/t3-provider-settings-unverified" || ! -f "$out/t3-provider-settings.png" || ! -f "$out/butler-provider-settings.png" ]]; then
      echo "Could not verify both requested native Providers settings frames" >&2
      exit 1
    fi
    compare_frames t3-provider-settings butler-provider-settings provider-settings
    printf '%s\n' \
      'About and Third-party notices were attempted in the same fresh native profiles after the verified Providers route.' \
      'Use provenance.txt for exact builds. A *-unverified marker means the corresponding PNG is diagnostic only.' \
      'Butler only displays its three bundled notices, not T3 Code’s full dependency manifest.' \
      >"$out/about-settings-capture-notes.txt"
    for state in about-settings open-source-licenses; do
      if [[ ! -e "$out/t3-$state-unverified" && ! -e "$out/butler-$state-unverified" &&
            -f "$out/t3-$state.png" && -f "$out/butler-$state.png" ]]; then
        if compare_frames "t3-$state" "butler-$state" "$state"; then
          printf '%s: verified route pair (raw diagnostic diff; not a parity claim)\n' "$state" \
            | tee -a "$out/about-settings-capture-notes.txt"
          continue
        fi
        touch "$out/$state-pair-unverified"
      fi
      printf '%s: optional native route pair not verified\n' "$state" \
        | tee -a "$out/about-settings-capture-notes.txt" >&2
    done
  fi
fi
if [[ -e "$out/t3-landing-unverified" ]]; then
  echo "T3 Code post-onboarding state was not verified" >&2
  exit 1
fi
if [[ "${VERIFY_BUTLER_NEW_PROJECT:-0}" == 1 ]]; then
  verify_native_new_project
fi
if [[ "${CAPTURE_T3_NEW_PROJECT_FORM:-0}" == 1 ]]; then
  verify_reference_new_project_form
fi
if [[ "${CAPTURE_GIT_URL_REVIEW:-0}" == 1 ]]; then
  verify_git_url_review_pairs
fi
if [[ "${VERIFY_BUTLER_THREAD_CHOOSER:-0}" == 1 ]]; then
  verify_native_thread_chooser
fi
if [[ -n "${CAPTURE_THREAD_FIND_DB:-}" ]]; then
  # Keep the main smoke verdict. capture_one has its own errexit, so do
  # not put it or its caller in an if/|| test (which suppresses errexit).
  set +e
  verify_native_thread_find "$CAPTURE_THREAD_FIND_DB"
  find_rc=$?
  set -e
  if (( find_rc != 0 )); then
    echo 'Optional native Thread Find did not complete; see unverified markers' >&2
  fi
fi
