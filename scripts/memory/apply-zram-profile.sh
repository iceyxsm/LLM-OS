#!/usr/bin/env bash
set -euo pipefail

PROFILE="${1:-balanced}"

case "$PROFILE" in
  balanced)
    ZRAM_FRACTION="1.0"
    ALGO="zstd"
    SWAPPINESS="100"
    ;;
  aggressive)
    ZRAM_FRACTION="1.5"
    ALGO="zstd"
    SWAPPINESS="180"
    ;;
  low-latency)
    ZRAM_FRACTION="0.5"
    ALGO="lz4"
    SWAPPINESS="60"
    ;;
  *)
    echo "unknown profile: $PROFILE" >&2
    exit 1
    ;;
esac

TOTAL_MEM_MB=$(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo)
ZRAM_SIZE_MB=$(awk -v mem="$TOTAL_MEM_MB" -v frac="$ZRAM_FRACTION" 'BEGIN {print int(mem*frac)}')

echo "Applying profile=$PROFILE mem=${TOTAL_MEM_MB}MB zram=${ZRAM_SIZE_MB}MB algo=$ALGO swappiness=$SWAPPINESS"

sudo modprobe zram
echo "$ALGO" | sudo tee /sys/block/zram0/comp_algorithm >/dev/null
echo "$((ZRAM_SIZE_MB*1024*1024))" | sudo tee /sys/block/zram0/disksize >/dev/null
sudo mkswap /dev/zram0
sudo swapon -p 100 /dev/zram0
sudo sysctl vm.swappiness="$SWAPPINESS"

echo "done"
