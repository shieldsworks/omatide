#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

if ! pkg-config --exists Qt6Core Qt6Gui Qt6Qml Qt6Network; then
  echo 'Qt6 QML dev packages not installed. Skipped.'
  exit 0
fi

ui_dir=$(sed -n 's/^[[:space:]]*readonly property string uiDir:[[:space:]]*//p' ui/Tide.qml)
repo_expr=$(sed -n 's/^[[:space:]]*readonly property string repo:[[:space:]]*//p' ui/Tide.qml)
parent_fn=$(awk '
  /^[[:space:]]*function parentOf\(/ { p = 1 }
  p { print }
  p && /^[[:space:]]*}[[:space:]]*$/ { exit }
' ui/Tide.qml)
start_script=$(awk -F\' '/const script = / { print $2; exit }' ui/Tide.qml)
[[ -n $ui_dir && -n $repo_expr && -n $parent_fn && -n $start_script ]] \
  || { echo 'ui/Tide.qml is missing uiDir, repo, parentOf, or the start script' >&2; exit 1; }
[[ $(printf '%s\n' "$ui_dir" | wc -l) -eq 1 && $(printf '%s\n' "$repo_expr" | wc -l) -eq 1 ]] \
  || { echo 'uiDir and repo must each be one line' >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export QT_QPA_PLATFORM=offscreen
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp/runtime-omatide-verify}
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export PATH=/usr/bin:/bin

g++ -fPIC -std=c++17 scripts/qs-url-probe.cpp -o "$work/qs-url-probe" \
  $(pkg-config --cflags --libs Qt6Core Qt6Gui Qt6Qml Qt6Network)

cat > "$work/probe.qml" <<EOF
import QtQuick
QtObject {
${parent_fn}
    readonly property string uiDir: ${ui_dir}
    readonly property string repo: ${repo_expr}
    Component.onCompleted: {
        console.log("REPO=" + repo)
        Qt.quit()
    }
}
EOF

failed=0
check() {
  local name=$1 file=$2 expect=$3
  local root=${4:-}
  local out got ran runtime log
  if [[ -n $root ]]; then
    out=$("$work/qs-url-probe" "$file" "$root" 2>&1) || true
  else
    out=$("$work/qs-url-probe" "$file" 2>&1) || true
  fi
  got=$(printf '%s\n' "$out" | sed -n 's/^qml: REPO=//p' | tail -n 1)
  runtime=$work/rt
  rm -rf "$runtime"
  mkdir -p "$runtime"
  log=$runtime/engine.log
  : > "$work/ran"
  RECORD=$work/ran sh -c "$start_script" omatide-start "$runtime" "$got/target/release/omatide" "$log" || true
  ran=$(cat "$work/ran" 2>/dev/null || true)
  printf '\n== %s\n' "$name"
  printf '%s\n' "$out" | sed -n 's/^/  /p'
  printf '  repo: %s\n  want: %s\n  started: %s\n' "$got" "$expect" "${ran:-<none>}"
  if [[ -s $log ]]; then
    printf '  log: %s\n' "$(tail -n 1 "$log")"
  fi
  if [[ $got != "$expect" || $ran != "$expect/target/release/omatide" ]]; then
    echo '  FAIL' >&2
    failed=1
  else
    echo '  ok'
  fi
}

mkdir -p "$work/checkout/ui" "$work/checkout/target/release"
cat > "$work/checkout/target/release/omatide" <<'EOF'
#!/bin/sh
printf '%s\n' "$0" > "$RECORD"
EOF
chmod +x "$work/checkout/target/release/omatide"
cp "$work/probe.qml" "$work/checkout/ui/probe.qml"
check 'checkout, quickshell -p ui/shell.qml' \
  "$work/checkout/ui/probe.qml" "$work/checkout"

mkdir -p "$work/shell/plugins/org.omahoy.tide/ui" \
  "$work/shell/plugins/org.omahoy.tide/target/release"
cp "$work/checkout/target/release/omatide" \
  "$work/shell/plugins/org.omahoy.tide/target/release/omatide"
cp "$work/probe.qml" "$work/shell/plugins/org.omahoy.tide/ui/probe.qml"
check 'plugin under the shell config root' \
  "$work/shell/plugins/org.omahoy.tide/ui/probe.qml" \
  "$work/shell/plugins/org.omahoy.tide" \
  "$work/shell"

if ! grep -q 'wrapMode: Text.Wrap' ui/TideWindow.qml \
  || ! grep -q 'engineStatus' ui/TideWindow.qml \
  || ! grep -q 'Tide.lastLog' ui/TideWindow.qml; then
  echo 'FAIL: the window does not wrap Tide.lastLog in the body' >&2
  failed=1
else
  echo 'window surfaces Tide.lastLog'
fi

if ((failed)); then
  echo 'FAILED' >&2
  exit 1
fi
echo 'All engine-path checks passed.'
