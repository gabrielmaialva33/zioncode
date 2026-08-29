#!/usr/bin/env bash
set -euo pipefail

JDK_VERSION="temurin-17.0.20+101"
NDK_VERSION="28.2.13676358"
SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_ROOT="$(cd "${SCRIPT_DIRECTORY}/.." && pwd -P)"
SDK_ROOT="${ANDROID_SDK_ROOT:-${XDG_DATA_HOME:-${HOME}/.local/share}/android-sdk}"
JNI_OUTPUT="${REPOSITORY_ROOT}/android/app/src/main/jniLibs"
GENERATED_LIBRARY="${JNI_OUTPUT}/arm64-v8a/libzion_android.so"
EXPECTED_LIBRARY="${REPOSITORY_ROOT}/android/app/src/main/jniLibs/arm64-v8a/libzion_android.so"

command -v realpath >/dev/null || {
    printf 'missing required command: realpath\n' >&2
    exit 1
}
if [[ "$(realpath --canonicalize-missing -- "${GENERATED_LIBRARY}")" != \
    "${EXPECTED_LIBRARY}" ]]; then
    printf 'refusing unsafe native-library output path: %s\n' "${GENERATED_LIBRARY}" >&2
    exit 1
fi

test -d "${SDK_ROOT}/ndk/${NDK_VERSION}" || {
    printf 'missing Android NDK %s under %s\n' "${NDK_VERSION}" "${SDK_ROOT}" >&2
    exit 1
}
test "$(cargo ndk --version)" = "cargo-ndk 4.1.2" || {
    printf 'cargo-ndk 4.1.2 is required\n' >&2
    exit 1
}

mkdir -p "${JNI_OUTPUT}"
# Absence after this exact-file removal proves the later artifact came from
# this cargo-ndk invocation. Never clean the surrounding jniLibs tree here.
rm -f -- "${GENERATED_LIBRARY}"
ANDROID_NDK_HOME="${SDK_ROOT}/ndk/${NDK_VERSION}" \
ANDROID_NDK_ROOT="${SDK_ROOT}/ndk/${NDK_VERSION}" \
cargo ndk \
    --target arm64-v8a \
    --platform 26 \
    --output-dir "${JNI_OUTPUT}" \
    build --release --package zion-android

if [[ ! -f "${GENERATED_LIBRARY}" || -L "${GENERATED_LIBRARY}" || \
    ! -s "${GENERATED_LIBRARY}" ]]; then
    printf 'cargo-ndk did not produce a fresh regular nonempty arm64 library: %s\n' \
        "${GENERATED_LIBRARY}" >&2
    exit 1
fi

if find "${JNI_OUTPUT}" \( -type f -o -type l \) -name '*.so' \
    ! -path "${GENERATED_LIBRARY}" |
    grep -q .; then
    printf 'unexpected non-arm64 native library under %s\n' "${JNI_OUTPUT}" >&2
    exit 1
fi

mise exec "java@${JDK_VERSION}" -- env \
    ANDROID_HOME="${SDK_ROOT}" \
    ANDROID_SDK_ROOT="${SDK_ROOT}" \
    "${REPOSITORY_ROOT}/android/gradlew" \
    --project-dir "${REPOSITORY_ROOT}/android" \
    --no-daemon \
    --dependency-verification strict \
    :app:testDebugUnitTest \
    :app:lint \
    :app:assembleDebug \
    :app:assembleRelease
