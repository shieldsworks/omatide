#!/usr/bin/env bash
# Resolves ui/Tide.qml's `repo` property the way `quickshell -p` loads it,
# then applies the same `[ -x ] || omatide` choice as Tide.start.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

if ! pkg-config --exists Qt6Core Qt6Gui Qt6Qml Qt6Network; then
  echo 'Qt6 QML dev packages not installed. Skipped.'
  exit 0
fi

repo_expr=$(sed -n 's/^[[:space:]]*readonly property string repo:[[:space:]]*//p' ui/Tide.qml)
[[ -n $repo_expr ]] || { echo 'ui/Tide.qml has no repo property' >&2; exit 1; }
[[ $(printf '%s\n' "$repo_expr" | wc -l) -eq 1 ]] || { echo 'repo property must be one line' >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export QT_QPA_PLATFORM=offscreen
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp/runtime-omatide-verify}
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

g++ -fPIC -std=c++17 scripts/qs-url-probe.cpp -o "$work/qs-url-probe" \
  $(pkg-config --cflags --libs Qt6Core Qt6Gui Qt6Qml Qt6Network)

cat > "$work/probe.qml" <<EOF
import QtQuick
QtObject {
    readonly property string repo: ${repo_expr}
    Component.onCompleted: {
        console.log("REPO=" + repo)
        Qt.quit()
    }
}
EOF

# Same choice as the start script in ui/Tide.qml.
pick() {
  local b=$1
  [ -x "$b" ] || b=omatide
  printf '%s\n' "$b"
}

failed=0
check() {
  local name=$1 file=$2 expect=$3
  local root=${4:-}
  local out got chosen
  if [[ -n $root ]]; then
    out=$("$work/qs-url-probe" "$file" "$root" 2>&1) || true
  else
    out=$("$work/qs-url-probe" "$file" 2>&1) || true
  fi
  got=$(printf '%s\n' "$out" | sed -n 's/^qml: REPO=//p' | tail -n 1)
  chosen=$(pick "$got/target/release/omatide")
  printf '\n== %s\n' "$name"
  printf '%s\n' "$out" | sed -n 's/^/  /p'
  printf '  repo: %s\n  want: %s\n  chosen: %s\n' "$got" "$expect" "$chosen"
  if [[ $got != "$expect" || $chosen != "$expect/target/release/omatide" ]]; then
    echo '  FAIL' >&2
    failed=1
  else
    echo '  ok'
  fi
}

mkdir -p "$work/checkout/ui" "$work/checkout/target/release"
printf '#!/bin/sh\nexit 0\n' > "$work/checkout/target/release/omatide"
chmod +x "$work/checkout/target/release/omatide"
cp "$work/probe.qml" "$work/checkout/ui/probe.qml"
check 'checkout, quickshell -p ui/shell.qml' \
  "$work/checkout/ui/probe.qml" "$work/checkout"

mkdir -p "$work/shell/plugins/org.omahoy.tide/ui" \
  "$work/shell/plugins/org.omahoy.tide/target/release"
printf '#!/bin/sh\nexit 0\n' > "$work/shell/plugins/org.omahoy.tide/target/release/omatide"
chmod +x "$work/shell/plugins/org.omahoy.tide/target/release/omatide"
cp "$work/probe.qml" "$work/shell/plugins/org.omahoy.tide/ui/probe.qml"
check 'plugin under the shell config root' \
  "$work/shell/plugins/org.omahoy.tide/ui/probe.qml" \
  "$work/shell/plugins/org.omahoy.tide" \
  "$work/shell"

if ! grep -q 'wrapMode: Text.Wrap' ui/TideWindow.qml \
  || ! grep -q 'engineError' ui/TideWindow.qml \
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
