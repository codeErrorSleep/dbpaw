#!/usr/bin/env bash
set -euo pipefail

install_prefix="${1:?Usage: $0 /absolute/install/prefix}"
case "$install_prefix" in
  /*) ;;
  *) echo "The install prefix must be an absolute path." >&2; exit 1 ;;
esac

# Build the Intel dependency with Xcode's cross compiler. Homebrew's current
# installer only supports Apple Silicon, including when run under Rosetta.
source_version="2.3.14"
source_sha256="4e2814de3e01fc30b0b9f75e83bb5aba91ab0384ee951286504bb70205524771"
build_dir="$(mktemp -d "${TMPDIR:-/tmp}/dbpaw-unixodbc.XXXXXX")"
trap 'rm -rf "$build_dir"' EXIT
archive="$build_dir/unixODBC.tar.gz"

curl --fail --location --retry 3 \
  "https://www.unixodbc.org/unixODBC-${source_version}.tar.gz" \
  --output "$archive"
printf '%s  %s\n' "$source_sha256" "$archive" | shasum -a 256 --check
tar -xzf "$archive" -C "$build_dir"
cd "$build_dir/unixODBC-${source_version}"

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-10.13}"
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export SDKROOT
CC="$(xcrun --find clang)" \
CXX="$(xcrun --find clang++)" \
CFLAGS="-arch x86_64" \
CXXFLAGS="-arch x86_64" \
LDFLAGS="-arch x86_64" \
  ./configure \
    --build="$(./config.guess)" \
    --host=x86_64-apple-darwin \
    --prefix="$install_prefix" \
    --sysconfdir=/usr/local/etc \
    --disable-gui \
    --disable-drivers \
    --disable-readline \
    --enable-shared \
    --disable-static \
    --with-included-ltdl

make -j "$(sysctl -n hw.ncpu)"
# Preserve the former Intel Homebrew config location without writing to the
# host's /usr/local/etc during installation or local verification.
staging_dir="$build_dir/staging"
make DESTDIR="$staging_dir" install
mkdir -p "$install_prefix"
cp -R "$staging_dir$install_prefix/." "$install_prefix/"

for library in libodbc.2.dylib libodbcinst.2.dylib; do
  lipo "$install_prefix/lib/$library" -verify_arch x86_64
  otool -L "$install_prefix/lib/$library"
done
