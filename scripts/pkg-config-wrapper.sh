#!/bin/sh

set -eu

find_real_pkg_config() {
  for candidate in /opt/homebrew/bin/pkg-config /usr/local/bin/pkg-config /usr/bin/pkg-config; do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

sdk_root() {
  if [ -n "${SDKROOT:-}" ]; then
    printf '%s\n' "$SDKROOT"
    return 0
  fi

  if command -v xcrun >/dev/null 2>&1; then
    xcrun --show-sdk-path 2>/dev/null || true
  fi
}

handle_libxml2_on_macos() {
  root="$(sdk_root)"

  if [ -z "$root" ] || [ ! -f "$root/usr/lib/libxml2.tbd" ]; then
    return 1
  fi

  requested_output=""

  for arg in "$@"; do
    case "$arg" in
      --modversion)
        printf '%s\n' "2.9.4"
        return 0
        ;;
      --libs)
        requested_output="$requested_output -L$root/usr/lib -lxml2"
        ;;
      --libs-only-L)
        requested_output="$requested_output -L$root/usr/lib"
        ;;
      --libs-only-l)
        requested_output="$requested_output -lxml2"
        ;;
      --cflags)
        requested_output="$requested_output -I$root/usr/include/libxml2"
        ;;
      --cflags-only-I)
        requested_output="$requested_output -I$root/usr/include/libxml2"
        ;;
      --exists|--silence-errors|--print-errors|--static)
        ;;
      --*)
        ;;
      *)
        ;;
    esac
  done

  printf '%s\n' "${requested_output# }"
}

requested_libxml2=false
for arg in "$@"; do
  if [ "$arg" = "libxml-2.0" ]; then
    requested_libxml2=true
    break
  fi
done

if [ "$requested_libxml2" = true ] && [ "$(uname -s)" = "Darwin" ]; then
  handle_libxml2_on_macos "$@" && exit 0
fi

if real_pkg_config="$(find_real_pkg_config)"; then
  unset PKG_CONFIG
  exec "$real_pkg_config" "$@"
fi

printf '%s\n' "pkg-config wrapper could not satisfy: $*" >&2
exit 1
