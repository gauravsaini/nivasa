#!/usr/bin/env bash
set -euo pipefail

# Coarse CI smoke gate: catches catastrophic benchmark regressions without pretending
# to replace the real Criterion output or a full historical baseline service.
max_upper_us="${BENCHMARK_DI_RESOLUTION_MAX_UPPER_US:-5000}"

output="$(
  cargo bench -p nivasa-benchmarks --bench di_resolution -- --quick --noplot 2>&1
)"

time_line="$(
  printf '%s\n' "$output" | awk '
    /di_resolution\/resolve_cached_singleton\/[0-9]+$/ {
      getline
      getline
      getline
      print
      exit
    }
  '
)"
if [ -z "$time_line" ]; then
  printf '%s\n' "$output"
  echo "failed to find Criterion time line for di_resolution/resolve_cached_singleton" >&2
  exit 1
fi

upper_value="$(printf '%s\n' "$time_line" | sed -E 's/.*\[[0-9.]+[[:space:]]+([^[:space:]]+)[[:space:]]+[0-9.]+[[:space:]]+([^[:space:]]+)[[:space:]]+([0-9.]+)[[:space:]]+([^[:space:]]+)\].*/\3 \4/')"
upper_num="${upper_value% *}"
upper_unit="${upper_value#* }"

upper_us="$(
  awk -v value="$upper_num" -v unit="$upper_unit" 'BEGIN {
    if (unit == "ns") print value / 1000.0;
    else if (unit == "us" || unit == "µs") print value;
    else if (unit == "ms") print value * 1000.0;
    else if (unit == "s") print value * 1000000.0;
    else exit 2;
  }'
)"

awk -v upper="$upper_us" -v limit="$max_upper_us" 'BEGIN {
  if (upper > limit) {
    printf("di_resolution/resolve_cached_singleton upper bound %.2f us exceeds budget %.2f us\n", upper, limit) > "/dev/stderr";
    exit 1;
  }
}'

printf '%s\n' "$time_line"
printf 'Benchmark budget ok: %.2f us <= %.2f us\n' "$upper_us" "$max_upper_us"
