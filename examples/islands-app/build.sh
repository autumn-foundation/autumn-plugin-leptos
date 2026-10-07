#!/usr/bin/env sh
# Builds the demo islands into ../islands/. Commit the output.
#
#   ./build.sh          build, then write ../islands/SOURCE.sha256
#   ./build.sh --check  fail when the sources changed after the last build
#
# Needs the `wasm32-unknown-unknown` target (see rust-toolchain.toml) and
# `cargo install wasm-bindgen-cli --version 0.2.129`.
#
# The wasm bytes depend on the checkout path (cargo puts it in the symbol
# hashes), so CI compares the source hash, not the bytes.
set -eu
cd "$(dirname "$0")"
here="$(pwd)"
repo="$(cd ../.. && pwd)"

source_hash() {
  (cd "$repo" && LC_ALL=C cat \
    Cargo.toml \
    examples/islands-app/build.sh \
    examples/islands-app/Cargo.toml \
    examples/islands-app/Cargo.lock \
    examples/islands-app/rust-toolchain.toml \
    examples/islands-app/src/*.rs \
    client/Cargo.toml \
    client/src/lib.rs | sha256sum | cut -d ' ' -f 1)
}

if [ "${1:-}" = "--check" ]; then
  if [ "$(cat ../islands/SOURCE.sha256)" != "$(source_hash)" ]; then
    echo "examples/islands/ is stale: run examples/islands-app/build.sh" >&2
    exit 1
  fi
  echo "examples/islands/ matches its sources"
  exit 0
fi

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
# Remove machine paths from panic messages.
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$repo=/repo" \
  CARGO_TARGET_DIR="$here/target" \
  cargo build --locked --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir ../islands \
  target/wasm32-unknown-unknown/release/leptos_demo_islands.wasm
source_hash > ../islands/SOURCE.sha256
ls -l ../islands
