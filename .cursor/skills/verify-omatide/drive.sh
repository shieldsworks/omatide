#!/usr/bin/env bash
set -euo pipefail
set -m

repo=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$repo"

run=
evidence=
bin=
engine_pid=
empty_pid=
window_pid=
build_start=
env_args=()
env_empty=()
self_pgid=$(ps -o pgid= -p $$ | tr -d ' ')
pid_list=
pgid_list=
held=

if [[ -n ${EVIDENCE_DIR:-} ]]; then
  mkdir -p "$EVIDENCE_DIR"
  evidence=$(mktemp -d "$EVIDENCE_DIR/drive.XXXXXX")
else
  evidence=$(mktemp -d /tmp/omatide-verify-evidence.XXXXXX)
fi

collect() {
  local pid=${1:-} child pgid
  [[ -n $pid ]] || return 0
  kill -0 "$pid" 2>/dev/null || return 0
  case " $pid_list " in
    *" $pid "*) ;;
    *) pid_list="$pid_list $pid" ;;
  esac
  pgid=$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ' || true)
  if [[ -n $pgid && $pgid != "$self_pgid" ]]; then
    case " $pgid_list " in
      *" $pgid "*) ;;
      *) pgid_list="$pgid_list $pgid" ;;
    esac
  fi
  for child in $(ps -o pid= --ppid "$pid" 2>/dev/null || true); do
    collect "$child"
  done
}

socket_inode() {
  local sock=$1
  [[ -S $sock ]] || return 0
  awk -v p="$sock" '$NF == p { print $7; exit }' /proc/net/unix
}

collect_socket() {
  local sock=$1 ino pid fd target
  ino=$(socket_inode "$sock")
  [[ -n ${ino:-} ]] || return 0
  for fd in /proc/[0-9]*/fd/*; do
    target=$(readlink "$fd" 2>/dev/null || true)
    if [[ $target == "socket:[$ino]" ]]; then
      pid=${fd#/proc/}
      pid=${pid%%/*}
      collect "$pid"
    fi
  done
}

signal_groups() {
  local sig=$1 pid pgid mine
  for pgid in $pgid_list; do
    kill "-$sig" -- "-$pgid" 2>/dev/null || true
  done
  for pid in $pid_list; do
    mine=$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ' || true)
    if [[ $mine == "$self_pgid" ]]; then
      kill "-$sig" "$pid" 2>/dev/null || true
    fi
  done
}

still_alive() {
  local pid pgid alive=0
  for pgid in $pgid_list; do
    if kill -0 -- "-$pgid" 2>/dev/null; then
      alive=1
    fi
  done
  for pid in $held; do
    if kill -0 "$pid" 2>/dev/null; then
      alive=1
    fi
  done
  [[ $alive -eq 1 ]]
}

stop_recorded() {
  local sock=${1:-} root=${2:-} i
  pid_list=
  pgid_list=
  collect "$root"
  if [[ -n $sock ]]; then
    collect_socket "$sock"
  fi
  held=$pid_list
  if [[ -z ${held// /} ]]; then
    if [[ -n $sock && -S $sock ]]; then
      rm -f "$sock"
    fi
    [[ -z $sock || ! -S $sock ]]
    return 0
  fi
  signal_groups TERM
  for i in 1 2 3 4 5 6 7 8 9 10; do
    if ! still_alive; then
      break
    fi
    sleep 0.1
  done
  signal_groups KILL
  for i in 1 2 3 4 5 6 7 8 9 10; do
    if ! still_alive; then
      break
    fi
    sleep 0.1
  done
  if [[ -n $root ]]; then
    wait "$root" 2>/dev/null || true
  fi
  if [[ -n $sock && -S $sock ]]; then
    pid_list=
    pgid_list=
    collect_socket "$sock"
    if [[ -n ${pid_list// /} ]]; then
      return 1
    fi
    rm -f "$sock"
  fi
  if still_alive; then
    return 1
  fi
  [[ -z $sock || ! -S $sock ]]
}

stop_all() {
  local child i
  pid_list=
  pgid_list=
  for child in $(ps -o pid= --ppid $$ 2>/dev/null || true); do
    collect "$child"
  done
  collect "${engine_pid:-}"
  collect "${empty_pid:-}"
  collect "${window_pid:-}"
  if [[ -n ${run:-} ]]; then
    collect_socket "$run/tide.sock" || true
    collect_socket "$run/empty.sock" || true
    collect_socket "$run/omatide/tide.sock" || true
  fi
  held=$pid_list
  [[ -n ${held// /} ]] || return 0
  signal_groups TERM
  for i in 1 2 3 4 5 6 7 8 9 10; do
    if ! still_alive; then
      break
    fi
    sleep 0.1
  done
  signal_groups KILL
  for i in 1 2 3 4 5 6 7 8 9 10; do
    if ! still_alive; then
      break
    fi
    sleep 0.1
  done
  wait || true
  return 0
}

archive_current() {
  local old have_run=0
  if [[ -n ${run:-} && -d ${run:-} && $run == /tmp/omatide-verify.* ]]; then
    have_run=1
  fi
  if [[ $have_run -eq 1 && -d $run/artifacts ]]; then
    cp -a "$run/artifacts/." "$evidence/"
  fi
  if [[ $have_run -eq 1 && -f $run/fetch.log ]]; then
    mkdir -p "$evidence/launch"
    cp -a "$run/fetch.log" "$evidence/launch/fetch.log"
  fi
  if [[ $have_run -eq 1 && -f $run/data/omatide/stations.json ]]; then
    mkdir -p "$evidence/launch"
    cp -a "$run/data/omatide/stations.json" "$evidence/launch/stations.json"
  fi
  if [[ $have_run -eq 1 && -f $run/engine.log ]]; then
    mkdir -p "$evidence/engine"
    cp -a "$run/engine.log" "$evidence/engine/engine.log"
  fi
  if [[ $have_run -eq 1 && -f $run/omatide/engine.log ]]; then
    mkdir -p "$evidence/window"
    cp -a "$run/omatide/engine.log" "$evidence/window/engine.log"
  fi
  stop_all
  engine_pid=
  empty_pid=
  window_pid=
  if [[ $have_run -eq 1 ]]; then
    old=$run
    run=
    rm -rf "$old"
  fi
}

finish() {
  local status=$?
  trap - EXIT INT TERM
  set +e
  archive_current
  printf 'evidence: %s\n' "$evidence"
  exit "$status"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

path_ok() {
  local val=$1 prefix rest resolved run_real part
  [[ -n ${run:-} && -d $run ]] || return 1
  [[ $val == /* ]] || return 1
  prefix=
  rest=${val#/}
  while [[ -n $rest ]]; do
    part=${rest%%/*}
    prefix=$prefix/$part
    if [[ -L $prefix && ! -e $prefix ]]; then
      return 1
    fi
    if [[ $rest == */* ]]; then
      rest=${rest#*/}
    else
      rest=
    fi
  done
  resolved=$(realpath -m "$val") || return 1
  run_real=$(realpath -m "$run") || return 1
  [[ $resolved == "$run_real" || $resolved == "$run_real"/* ]]
}

env_ok() {
  local file=$1 key line val n
  [[ -n ${run:-} && -n $file && -r $file && -s $file ]] || return 1
  while IFS= read -r line || [[ -n ${line:-} ]]; do
    case $line in
      HOME=*|XDG_RUNTIME_DIR=*|XDG_DATA_HOME=*|XDG_CONFIG_HOME=*|OMATIDE_DATA_DIR=*|OMATIDE_CONFIG=*|TZ=*) ;;
      *) return 1 ;;
    esac
  done < "$file"
  for key in HOME XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME OMATIDE_DATA_DIR OMATIDE_CONFIG TZ; do
    n=$(grep -c "^${key}=" "$file" || true)
    [[ $n -eq 1 ]] || return 1
    val=$(sed -n "s/^${key}=//p" "$file")
    [[ -n $val ]] || return 1
    case $val in
      *[[:space:]]*|*\**|*\?*|*\[*|*\]*) return 1 ;;
    esac
    if [[ $key != TZ ]]; then
      path_ok "$val" || return 1
    fi
  done
}

load_file() {
  local -n dest=$1
  local file=$2 line
  env_ok "$file" || return 1
  dest=()
  while IFS= read -r line || [[ -n ${line:-} ]]; do
    [[ -n $line ]] || continue
    dest+=("$line")
  done < "$file"
}

record() {
  local id=$1 dir code
  shift
  dir=$run/artifacts/verify/$id
  mkdir -p "$dir"
  printf '%q ' "$@" >"$dir/command.txt"
  printf '\n' >>"$dir/command.txt"
  set +e
  "$@" >"$dir/stdout.txt" 2>"$dir/stderr.txt"
  code=$?
  set -e
  printf '%s\n' "$code" >"$dir/exit-code.txt"
  [[ -s $dir/stdout.txt ]] || rm -f "$dir/stdout.txt"
  [[ -s $dir/stderr.txt ]] || rm -f "$dir/stderr.txt"
  return 0
}

need() {
  local id=$1 want=$2 got
  got=$(cat "$run/artifacts/verify/$id/exit-code.txt")
  [[ $got -eq $want ]]
}

logged() {
  local id=$1 needle=$2 stream=${3:-stdout.txt}
  [[ -f $run/artifacts/verify/$id/$stream ]] || return 1
  grep -F -q "$needle" "$run/artifacts/verify/$id/$stream"
}

launch() {
  local pin=1.98.1 compiler ver mtime
  build_start=$(date +%s)
  touch src/main.rs
  rm -f target/release/omatide
  if command -v mise >/dev/null 2>&1; then
    mise install
    mise exec -- cargo build --release --locked
    compiler=$(mise exec -- rustc --version)
  else
    command -v rustup >/dev/null 2>&1 || {
      printf '%s\n' 'neither mise nor rustup is on PATH' >&2
      exit 1
    }
    rustup toolchain install "$pin"
    rustup run "$pin" cargo build --release --locked
    compiler=$(rustup run "$pin" rustc --version)
  fi
  [[ $compiler == *"$pin"* ]] || {
    printf '%s\n' "compiler is $compiler, want $pin" >&2
    exit 1
  }
  bin=$(realpath ./target/release/omatide)
  ver=$(env -i "$bin" --version)
  [[ $ver == "omatide 0.1.0" ]]
  mtime=$(stat -c %Y "$bin")
  [[ $mtime -ge $build_start ]] || {
    printf '%s\n' "release binary mtime $mtime is older than build start $build_start" >&2
    exit 1
  }
  run=$(mktemp -d /tmp/omatide-verify.XXXXXX)
  [[ $run == /tmp/omatide-verify.* ]]
  mkdir -p "$run/home" "$run/empty" "$run/artifacts/verify/launch"
  printf '%s\n' "$build_start" >"$run/artifacts/verify/launch/build_start"
  printf '%s\n' "$mtime" >"$run/artifacts/verify/launch/bin_mtime"
  printf '%s\n' "mtime $mtime" >"$run/artifacts/verify/launch/fresh.txt"
  printf '%s\n' "$compiler" >"$run/artifacts/verify/launch/rustc.txt"
  cat >"$run/env" <<EOF
HOME=$run/home
XDG_RUNTIME_DIR=$run
XDG_DATA_HOME=$run/data
XDG_CONFIG_HOME=$run/config
OMATIDE_DATA_DIR=$run/data/omatide
OMATIDE_CONFIG=$run/config.toml
TZ=UTC
EOF
  grep -v '^OMATIDE_DATA_DIR=' "$run/env" >"$run/env.empty"
  printf 'OMATIDE_DATA_DIR=%s\n' "$run/empty" >>"$run/env.empty"
  load_file env_args "$run/env"
  load_file env_empty "$run/env.empty"
  env "${env_args[@]}" "$bin" fetch --region 37.80,-122.48,37.82,-122.45 \
    >"$run/fetch.log" 2>&1
  [[ -f $run/data/omatide/stations.json ]]
  [[ ! -e $run/empty/stations.json ]]
  grep -F -q '2 stations, 2 of them predictable' "$run/fetch.log"
  grep -F -q "saved to $run/data/omatide/stations.json" "$run/fetch.log"
  printf 'fresh: mtime %s\n' "$mtime"
  printf 'compiler: %s\n' "$compiler"
}

drive_doctor() {
  local data lines
  record doctor env "${env_args[@]}" "$bin" --version
  need doctor 0
  logged doctor 'omatide 0.1.0'
  data=$(sed -n 's/^OMATIDE_DATA_DIR=//p' "$run/env")
  [[ -f $data/stations.json ]]
  lines=$(grep -c . "$run/env")
  [[ $lines -eq 7 ]]
  [[ $(tail -n 1 "$run/env.empty") == "OMATIDE_DATA_DIR=$run/empty" ]]
  [[ ! -e $run/empty/stations.json ]]
  printf 'doctor: 0\n'
  cat "$run/artifacts/verify/doctor/stdout.txt"
}

drive_tide() {
  record tide-station env "${env_args[@]}" "$bin" tide 9414290
  need tide-station 0
  logged tide-station '9414290'
  logged tide-station 'SAN FRANCISCO (Golden Gate)'
  logged tide-station 'm above chart datum'
  logged tide-station 'high'
  logged tide-station 'low'
  record tide-home env "${env_args[@]}" "$bin" tide
  need tide-home 0
  logged tide-home '9414290'
  record tide-pos env "${env_args[@]}" "$bin" tide 37.8063,-122.4659
  need tide-pos 0
  logged tide-pos '9414290'
  record tide-stations env "${env_args[@]}" "$bin" stations
  need tide-stations 0
  logged tide-stations '9414290'
  logged tide-stations 'SAN FRANCISCO (Golden Gate)'
  record tide-unknown env "${env_args[@]}" "$bin" tide SFB9999
  need tide-unknown 1
  logged tide-unknown 'omatide: no station SFB9999' stderr.txt
  record tide-missing env "${env_empty[@]}" "$bin" tide 9414290
  need tide-missing 1
  logged tide-missing 'Run `omatide fetch` first.' stderr.txt
  grep -F -q '"id": "9414290"' "$run/data/omatide/stations.json"
  printf 'tide: 0\n'
}

drive_current() {
  record current-home env "${env_args[@]}" "$bin" current
  need current-home 0
  logged current-home 'PCT0286-1'
  logged current-home 'kn'
  logged current-home 'slack'
  record current-bin env "${env_args[@]}" "$bin" current PCT0286-1
  need current-bin 0
  logged current-bin 'PCT0286-1'
  record current-list env "${env_args[@]}" "$bin" stations current
  need current-list 0
  logged current-list 'PCT0286-1'
  if logged current-list '9414290'; then
    printf '%s\n' 'current list named a tide station' >&2
    exit 1
  fi
  record current-missing env "${env_empty[@]}" "$bin" current
  need current-missing 1
  logged current-missing 'Run `omatide fetch` first.' stderr.txt
  grep -F -q '"id": "PCT0286"' "$run/data/omatide/stations.json"
  printf 'current: 0\n'
}

drive_bay() {
  record bay-at env "${env_args[@]}" "$bin" bay --at 2026-09-20T17:00:00Z
  need bay-at 0
  logged bay-at 'San Francisco Bay at 2026-09-20T17:00:00Z'
  logged bay-at 'Golden Gate'
  logged bay-at 'The stream:'
  printf 'bay: 0\n'
}

wait_socket() {
  local sock=$1 pid=$2 i
  for i in $(seq 50); do
    if [[ -S $sock ]] && kill -0 "$pid" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

drive_engine() {
  env "${env_args[@]}" "$bin" run --socket "$run/tide.sock" --no-keel \
    >"$run/engine.log" 2>&1 &
  engine_pid=$!
  wait_socket "$run/tide.sock" "$engine_pid"
  record engine-watch timeout 2 env "${env_args[@]}" "$bin" watch --socket "$run/tide.sock"
  need engine-watch 124
  logged engine-watch '{"tide":"0.1.0","type":"hello","v":1}'
  logged engine-watch '"keel":"off"'
  logged engine-watch '"station":"9414290"'
  record engine-second env "${env_args[@]}" "$bin" run --socket "$run/tide.sock" --no-keel
  need engine-second 1
  logged engine-second "already running on $run/tide.sock" stderr.txt
  kill -0 "$engine_pid"
  record engine-watch2 timeout 2 env "${env_args[@]}" "$bin" watch --socket "$run/tide.sock"
  need engine-watch2 124
  logged engine-watch2 '{"tide":"0.1.0","type":"hello","v":1}'
  env "${env_empty[@]}" "$bin" run --socket "$run/empty.sock" --no-keel \
    >"$run/empty.log" 2>&1 &
  empty_pid=$!
  wait_socket "$run/empty.sock" "$empty_pid"
  record engine-empty-watch timeout 2 env "${env_empty[@]}" "$bin" watch --socket "$run/empty.sock"
  need engine-empty-watch 124
  grep -F -q 'run `omatide fetch`' "$run/empty.log"
  logged engine-empty-watch '"stations":{"current":0,"tide":0}'
  if logged engine-empty-watch '"station":"9414290"'; then
    printf '%s\n' 'empty catalog still named 9414290' >&2
    exit 1
  fi
  kill -0 "$empty_pid"
  printf 'engine: 0\n'
}

resolve_wayland() {
  local display=${WAYLAND_DISPLAY-} runtime=${XDG_RUNTIME_DIR-} socket= dir
  dir=$run/artifacts/verify/window-open
  mkdir -p "$dir"
  wayland_abs=
  case $display in
    "")
      printf '%s\n' 'WAYLAND_DISPLAY is empty' 'not run: ./run.sh' >"$dir/wayland.txt"
      return 1
      ;;
    /*) socket=$display ;;
    *)
      case $runtime in
        /*) socket=$runtime/$display ;;
        *)
          printf '%s\n' 'XDG_RUNTIME_DIR is not absolute' 'not run: ./run.sh' >"$dir/wayland.txt"
          return 1
          ;;
      esac
      ;;
  esac
  if [[ -S $socket ]]; then
    wayland_abs=$socket
    printf '%s\n' "$wayland_abs" >"$dir/wayland.txt"
    return 0
  fi
  printf '%s\n' "wayland socket is not reachable: $socket" 'not run: ./run.sh' >"$dir/wayland.txt"
  return 1
}

drive_window() {
  local probe i code dir start_script
  local -a cmd
  probe=$(env "${env_args[@]}" sh -c 'printf %s "$XDG_RUNTIME_DIR"')
  [[ $probe == "$run" ]]
  dir=$run/artifacts/verify/window-open
  mkdir -p "$dir"
  if resolve_wayland; then
    cmd=("${env_args[@]}" "WAYLAND_DISPLAY=$wayland_abs")
    printf '%q ' env "${cmd[@]}" ./run.sh >"$dir/command.txt"
    printf '\n' >>"$dir/command.txt"
    env "${cmd[@]}" ./run.sh >"$dir/stdout.txt" 2>"$dir/stderr.txt" &
    window_pid=$!
    for i in $(seq 30); do
      if ! kill -0 "$window_pid" 2>/dev/null; then
        break
      fi
      sleep 0.1
    done
    if kill -0 "$window_pid" 2>/dev/null; then
      ps -o args= -p "$window_pid" >"$dir/command-ps.txt" || true
      printf '%s\n' 124 >"$dir/exit-code.txt"
      stop_recorded "$run/omatide/tide.sock" "$window_pid"
      window_pid=
      printf 'window-open: 124\n'
    else
      set +e
      wait "$window_pid"
      code=$?
      set -e
      window_pid=
      printf '%s\n' "$code" >"$dir/exit-code.txt"
      printf 'window-open: %s\n' "$code" >&2
      return 1
    fi
  else
    printf '%s\n' 1 >"$dir/exit-code.txt"
    printf '%s\n' 'not driven' >"$dir/miss.txt"
    [[ ! -e $dir/stdout.txt ]]
    printf 'window-open: not-driven\n'
  fi

  start_script='mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"'
  dir=$run/artifacts/verify/window-start
  mkdir -p "$dir"
  env "${env_args[@]}" env -C "$run/home" sh -c "$start_script" omatide-start \
    "$run/omatide/" "$bin" "$run/omatide/engine.log" \
    >"$dir/stdout.txt" 2>"$dir/stderr.txt" &
  window_pid=$!
  wait_socket "$run/omatide/tide.sock" "$window_pid"
  ps -o args= -p "$window_pid" >"$dir/command-ps.txt"
  record window-watch timeout 2 env "${env_args[@]}" "$bin" watch --socket "$run/omatide/tide.sock"
  need window-watch 124
  logged window-watch '{"tide":"0.1.0","type":"hello","v":1}'
  logged window-watch '"keel":"connecting"'
  record window-second env "${env_args[@]}" "$bin" run
  need window-second 1
  logged window-second "already running on $run/omatide/tide.sock" stderr.txt
  stop_recorded "$run/omatide/tide.sock" "$window_pid"
  window_pid=
  [[ ! -S $run/omatide/tide.sock ]]
  printf 'window-start: stopped\n'

  record window-engine ./scripts/verify.sh
  need window-engine 0
  if logged window-engine 'Skipped.' || logged window-engine 'Skipped.' stderr.txt; then
    printf '%s\n' 'scripts/verify.sh skipped' >&2
    exit 1
  fi
  if ! logged window-engine 'All engine-path checks passed.' \
    && ! logged window-engine 'All engine-path checks passed.' stderr.txt; then
    printf '%s\n' 'scripts/verify.sh did not pass' >&2
    exit 1
  fi

  dir=$run/artifacts/verify/window-missing
  mkdir -p "$dir"
  set +e
  env "${env_args[@]}" env -C "$run/home" env PATH=/usr/bin:/bin \
    sh -c "$start_script" omatide-start "$run/omatide/" "$run/not-a-binary" \
    "$run/omatide/engine.log" >"$dir/stdout.txt" 2>"$dir/stderr.txt"
  code=$?
  set -e
  printf '%s\n' "$code" >"$dir/exit-code.txt"
  [[ $code -eq 127 ]]
  [[ -f $run/omatide/engine.log ]]
  grep -F -q 'exec: omatide: not found' "$run/omatide/engine.log"
  printf 'window-missing: 0\n'
}

if [[ $# -eq 0 ]]; then
  set -- doctor tide current bay engine window
fi

for feature in "$@"; do
  case $feature in
    doctor|tide|current|bay|engine|window) ;;
    *)
      printf 'unknown feature: %s\n' "$feature" >&2
      exit 2
      ;;
  esac
done

launch
for feature in "$@"; do
  "drive_$feature"
done
