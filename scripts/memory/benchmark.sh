#!/usr/bin/env bash
set -euo pipefail

STAMP=$(date +%Y%m%d_%H%M%S)
OUT_DIR="runtime/memory-manager/results"
OUT_FILE="$OUT_DIR/benchmark_$STAMP.txt"
mkdir -p "$OUT_DIR"

{
  echo "timestamp=$STAMP"
  echo "--- free -h ---"
  free -h
  echo
  echo "--- /proc/swaps ---"
  cat /proc/swaps
  echo
  echo "--- zram stats ---"
  if [ -d /sys/block/zram0 ]; then
    cat /sys/block/zram0/mm_stat || true
  else
    echo "zram0 not found"
  fi
} | tee "$OUT_FILE"

echo "written $OUT_FILE"
