# Fixed, non-interactive POSIX script. Only raw counters cross the SSH boundary.
LC_ALL=C
export LC_ALL
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH
section() { printf '\n__FN_METRIC_%s__\n' "$1"; }
file() {
  section "$1"
  if [ -r "$2" ]; then
    while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$2"
  else printf '!permission_denied\n'; fi
}
command_metric() {
  section "$1"
  shift
  if command -v "$1" >/dev/null 2>&1; then
    "$@" 2>/dev/null || printf '\n!collection_failed\n'
  else printf '!missing_command\n'; fi
}
section platform
platform=$(uname -s 2>/dev/null)
printf '%s\n' "$platform"
case "$platform" in
  Linux)
    section cpu
    if [ -r /proc/stat ]; then
      IFS= read -r cpu_line < /proc/stat
      printf '%s\n' "$cpu_line"
    else printf '!permission_denied\n'; fi
    file memory /proc/meminfo
    file uptime /proc/uptime
    ;;
  Darwin)
    command_metric cpu top -l 2 -s 1 -n 0
    command_metric memory vm_stat
    command_metric total sysctl -n hw.memsize
    command_metric boot sysctl -n kern.boottime
    command_metric now date +%s
    ;;
esac
section disk
if command -v df >/dev/null 2>&1; then
  df -kP / 2>/dev/null || df -k / 2>/dev/null || printf '\n!collection_failed\n'
else printf '!missing_command\n'; fi
section end
