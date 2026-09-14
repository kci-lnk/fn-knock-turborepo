# Fixed POSIX script. All parsing stays on the server; no PTY is allocated.
LC_ALL=C
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export LC_ALL PATH
printf '__FN_METRIC_disks__\n'
if command -v df >/dev/null 2>&1; then
  df -kP 2>/dev/null || df -k 2>/dev/null || printf '\n!collection_failed\n'
else
  printf '!missing_command\n'
fi
printf '\n__FN_METRIC_end__\n'
