#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_PROFILE_RELEASE_STRIP=symbols CARGO_PROFILE_RELEASE_DEBUG=0 \
  cargo build --locked --release --target x86_64-pc-windows-gnu -p comicgpu-desktop
build_dir="$(pwd)/target/x86_64-pc-windows-gnu/release"
export COMICGPU_APP_EXE="$build_dir/comicgpu-desktop.exe"
export COMICGPU_WEBVIEW_LOADER="$build_dir/WebView2Loader.dll"
mkdir -p dist
printf '1 ICON "%s"\n' "$(pwd)/crates/desktop/icons/icon.ico" > "$build_dir/portable.rc"
x86_64-w64-mingw32-windres "$build_dir/portable.rc" -o "$build_dir/portable-icon.o"
rustc --edition=2021 --target x86_64-pc-windows-gnu -C opt-level=2 -C strip=symbols \
  -C "link-arg=$build_dir/portable-icon.o" tools/portable-launcher.rs \
  -o dist/ComicGPU-Windows-0.2.0-x64.exe
