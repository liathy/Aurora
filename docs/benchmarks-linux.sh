#!/bin/bash
# Aurora vs official Steam on this PC, one client at a time.
# Per run: start -> main window shown, then 45 s idle, then memory (PSS of
# every process the client started), process count, and CPU over 30 s.
# Aurora's window is found through X11 (it runs on XWayland here); Steam's
# is a Wayland window, so its own log line "SP Desktop_uid0-'Steam':
# WasHidden 0" (main window shown) is the signal, polled every 50 ms.
set -u
export DISPLAY=:0
OUT=${OUT:-/dev/stdout}
AURORA_DIR=$1
RUNS=${RUNS:-4}
S=$HOME/.local/share/Steam
WEBLOG=$S/logs/webhelper.txt
HOOKS=("$S/ubuntu12_64/libXtst.so.6" "$S/steamrt64/libXtst.so.6")

restore() {
  for h in "${HOOKS[@]}"; do [ -L "$h.bench-off" ] && mv "$h.bench-off" "$h"; done
}
trap restore EXIT

# Every process under `root`, plus Steam's runtime loggers (re-parented
# out of the tree) started after it.
pids_of() {
  ps -eo pid=,ppid=,comm= | awk -v r="$1" '{p[$1]=$2; c[$1]=$3} END{for(x in p){y=x; while(y>1){ if(y==r){print x; break} y=p[y]} if(c[x]=="srt-logger" && x+0>r+0) print x}}' | sort -u
}

pss_mb() {
  local t=0 v
  for p in $(pids_of "$1"); do
    v=$(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null); t=$((t + ${v:-0}))
  done
  echo $((t / 1024))
}

ticks() {
  local t=0 v
  for p in $(pids_of "$1"); do
    v=$(awk '{print $14+$15}' /proc/$p/stat 2>/dev/null); t=$((t + ${v:-0}))
  done
  echo $t
}

wait_aurora_picker() {
  local deadline=$((SECONDS + 120))
  while [ $SECONDS -lt $deadline ]; do
    xdotool search --onlyvisible --name '^Aurora$' >/dev/null 2>&1 && return 0
    sleep 0.05
  done
  return 1
}

wait_aurora() {
  local deadline=$((SECONDS + 120)) last_click=-5 wd
  while [ $SECONDS -lt $deadline ]; do
    # Settings ask "Who's playing?" first (a 543 px window): pick the
    # account as soon as it shows, then wait for the main window.
    for w in $(xdotool search --onlyvisible --name '^Aurora$' 2>/dev/null); do
      wd=$(xdotool getwindowgeometry --shell "$w" 2>/dev/null | awk -F= '/WIDTH/{print $2}')
      [ "${wd:-0}" -gt 700 ] && return 0
      if [ $((SECONDS - last_click)) -ge 1 ]; then xdotool mousemove --window "$w" 205 205 click 1; last_click=$SECONDS; fi
    done
    sleep 0.05
  done
  return 1
}

wait_steam_log() { # byte offset of webhelper.txt before the start, pattern, timeout
  local deadline=$((SECONDS + $3)) from=$1 size
  while [ $SECONDS -lt $deadline ]; do
    size=$(stat -c %s "$WEBLOG" 2>/dev/null || echo 0)
    [ "$size" -lt "$from" ] && from=0 # rotated
    tail -c +$((from + 1)) "$WEBLOG" 2>/dev/null | grep -q "$2" && return 0
    sleep 0.05
  done
  return 1
}

run() { # client run-number
  local client=$1 n=$2 root t0 t1 t2 main win off mem procs a b cpu ok
  off=$(stat -c %s "$WEBLOG" 2>/dev/null || echo 0)
  t0=$(date +%s.%N)
  if [ "$client" = aurora ]; then
    (cd "$AURORA_DIR" && exec env WAYLAND_DISPLAY= setsid ./aurora >/dev/null 2>&1) &
    root=$!
    wait_aurora_picker && ok=1 || ok=0
    t1=$(date +%s.%N)
    wait_aurora && t2=$(date +%s.%N) || t2=$t0
  else
    setsid steam >/dev/null 2>&1 &
    root=$!
    # First window: the sign-in window, or the main one with auto-login.
    wait_steam_log "$off" "SP DesktopLoginWindow_uid0-.*WasHidden 0\|SP Desktop_uid0-'Steam': WasHidden 0" 120 && ok=1 || ok=0
    t1=$(date +%s.%N)
    # If it asked, the user picks their account; then the main window.
    wait_steam_log "$off" "SP Desktop_uid0-'Steam': WasHidden 0" 300 && t2=$(date +%s.%N) || t2=$t0
  fi
  if [ $ok = 1 ]; then win=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.2f", b-a}'); else win=timeout; fi
  main=$(awk -v a="$t0" -v b="$t2" 'BEGIN{printf "%.2f", b-a}')
  sleep 45
  mem=$(pss_mb "$root")
  procs=$(pids_of "$root" | wc -l)
  a=$(ticks "$root"); sleep 30; b=$(ticks "$root")
  cpu=$(awk -v a="$a" -v b="$b" -v hz="$(getconf CLK_TCK)" 'BEGIN{printf "%.2f", (b-a)*100/hz/30}')
  echo "$client,$n,$win,$main,$mem,$procs,$cpu" >> "$OUT"
  if [ "$client" = aurora ]; then kill $(pids_of "$root") 2>/dev/null; else steam -shutdown >/dev/null 2>&1; fi
  for _ in $(seq 120); do [ -z "$(pids_of "$root")" ] && break; sleep 0.5; done
  kill -9 $(pids_of "$root") 2>/dev/null
  sleep 5
}

echo "client,run,signin_window_s,main_window_s,pss_mb,processes,cpu_pct_of_core" >> "$OUT"
# Plain Steam first (the user is clicking): without the Millennium mod.
for h in "${HOOKS[@]}"; do [ -L "$h" ] && mv "$h" "$h.bench-off"; done
for n in $(seq 0 $((RUNS - 1))); do run steam "$n"; done
restore
for n in $(seq 0 $((RUNS - 1))); do run aurora "$n"; done
echo done >> "$OUT"
