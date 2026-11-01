#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBMODULE_DIR="${ROOT_DIR}/vendor/dosbox-staging"
PATCH_FILE="${ROOT_DIR}/patches/dosbox-staging-screamer-hooks.patch"
BUILD_DIR="${ROOT_DIR}/target/dosbox-staging-build"
OUT_DIR="${ROOT_DIR}/target/release"

export PATH="${HOME}/.local/bin:${PATH}"

if [[ ! -f "${SUBMODULE_DIR}/meson.build" ]]; then
    echo "==> Initializing vendor/dosbox-staging submodule..."
    git -C "${ROOT_DIR}" submodule update --init --depth 1 vendor/dosbox-staging
fi

if git -C "${SUBMODULE_DIR}" apply --check "${PATCH_FILE}" 2>/dev/null; then
    echo "==> Applying source-level hook patch to vendor/dosbox-staging..."
    git -C "${SUBMODULE_DIR}" apply "${PATCH_FILE}"
else
    echo "==> Source-level hook patch already applied in vendor/dosbox-staging."
fi

echo "==> Building static libscreamer_hooks.a..."
cargo build --release -p screamer-hooks --manifest-path "${ROOT_DIR}/Cargo.toml"

if [[ ! -f "${BUILD_DIR}/build.ninja" ]]; then
    rm -rf "${BUILD_DIR}"
    echo "==> Configuring dosbox-staging with meson..."
    meson setup "${BUILD_DIR}" "${SUBMODULE_DIR}" \
        -Dbuildtype=release \
        -Ddefault_library=static \
        -Duse_sdl2_net=false \
        -Duse_mt32emu=false \
        -Duse_slirp=false \
        -Duse_fluidsynth=false \
        -Duse_zlib_ng=false \
        -Dunit_tests=disabled
fi

echo "==> Compiling dosbox-staging with static libscreamer_hooks.a..."
ninja -C "${BUILD_DIR}" dosbox

mkdir -p "${OUT_DIR}"
cp -f "${BUILD_DIR}/dosbox" "${OUT_DIR}/dosbox"
rm -rf "${OUT_DIR}/resources"
cp -r "${SUBMODULE_DIR}/contrib/resources" "${OUT_DIR}/resources"

echo "==> Built ${OUT_DIR}/dosbox (with bundled resources in ${OUT_DIR}/resources)"
