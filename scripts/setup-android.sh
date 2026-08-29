#!/usr/bin/env bash
set -euo pipefail

JDK_VERSION="temurin-17.0.20+101"
COMMAND_LINE_TOOLS_BUILD="15859902"
COMMAND_LINE_TOOLS_SHA256="4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583"
NDK_VERSION="28.2.13676358"
PLATFORM_TOOLS_VERSION="37.0.1"
# Google's repository2-3.xml publishes the exact URL, byte count, and SHA-1.
# SHA-256 additionally pins the bytes downloaded after that official check.
PLATFORM_TOOLS_BYTES="9054187"
PLATFORM_TOOLS_SHA1="477254aa5f903c15cf51001717bdf347fb6b53e0"
PLATFORM_TOOLS_SHA256="d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1"
PLATFORM_TOOLS_URL="https://dl.google.com/android/repository/platform-tools_r37.0.1-linux.zip"
SDK_ROOT="${ANDROID_SDK_ROOT:-${XDG_DATA_HOME:-${HOME}/.local/share}/android-sdk}"
SDK_MANAGER="${SDK_ROOT}/cmdline-tools/latest/bin/sdkmanager"
COMMAND_LINE_TOOLS_PROPERTIES="${SDK_ROOT}/cmdline-tools/latest/source.properties"
PLATFORM_TOOLS_PROPERTIES="${SDK_ROOT}/platform-tools/source.properties"

for required in mise curl bsdtar sha1sum sha256sum stat rustup cargo; do
    command -v "${required}" >/dev/null || {
        printf 'missing required command: %s\n' "${required}" >&2
        exit 1
    }
done

setup_tmp="$(mktemp -d /tmp/zion-android-sdk.XXXXXX)"
trap 'rm -rf -- "${setup_tmp}"' EXIT

# `mise install` installs without activating or changing the global Java version.
mise install "java@${JDK_VERSION}"

if [[ ! -x "${SDK_MANAGER}" ]]; then
    if [[ -e "${SDK_ROOT}/cmdline-tools/latest" ]]; then
        printf 'refusing to replace existing command-line tools: %s\n' \
            "${SDK_ROOT}/cmdline-tools/latest" >&2
        exit 1
    fi
    archive="${setup_tmp}/command-line-tools.zip"
    curl --fail --location \
        --output "${archive}" \
        "https://dl.google.com/android/repository/commandlinetools-linux-${COMMAND_LINE_TOOLS_BUILD}_latest.zip"
    printf '%s  %s\n' "${COMMAND_LINE_TOOLS_SHA256}" "${archive}" |
        sha256sum --check --status
    mkdir -p "${SDK_ROOT}/cmdline-tools"
    bsdtar -xf "${archive}" -C "${setup_tmp}"
    mv "${setup_tmp}/cmdline-tools" "${SDK_ROOT}/cmdline-tools/latest"
fi

test "$(sed -n 's/^Pkg.Revision *= *//p' "${COMMAND_LINE_TOOLS_PROPERTIES}")" = "22.0" || {
    printf 'Android command-line tools 22.0 (archive build %s) are required\n' \
        "${COMMAND_LINE_TOOLS_BUILD}" >&2
    exit 1
}

# sdkmanager remains the documented headless installer for the selected tools release.
set +o pipefail
yes | mise exec "java@${JDK_VERSION}" -- \
    "${SDK_MANAGER}" --sdk_root="${SDK_ROOT}" --licenses
sdk_license_status="${PIPESTATUS[1]}"
set -o pipefail
if [[ "${sdk_license_status}" -ne 0 ]]; then
    printf 'Android SDK license acceptance failed\n' >&2
    exit "${sdk_license_status}"
fi

installed_platform_tools="$({
    sed -n 's/^Pkg.Revision *= *//p' "${PLATFORM_TOOLS_PROPERTIES}" 2>/dev/null || true
} | head -n 1)"
if [[ "${installed_platform_tools}" != "${PLATFORM_TOOLS_VERSION}" ]]; then
    if [[ -e "${SDK_ROOT}/platform-tools" || -L "${SDK_ROOT}/platform-tools" ]]; then
        printf 'refusing to replace Android platform-tools revision %s under %s\n' \
            "${installed_platform_tools:-unknown}" "${SDK_ROOT}" >&2
        exit 1
    fi
    platform_archive="${setup_tmp}/platform-tools.zip"
    curl --fail --location --output "${platform_archive}" "${PLATFORM_TOOLS_URL}"
    test "$(stat --format='%s' "${platform_archive}")" = "${PLATFORM_TOOLS_BYTES}" || {
        printf 'Android platform-tools archive size verification failed\n' >&2
        exit 1
    }
    printf '%s  %s\n' "${PLATFORM_TOOLS_SHA1}" "${platform_archive}" |
        sha1sum --check --status
    printf '%s  %s\n' "${PLATFORM_TOOLS_SHA256}" "${platform_archive}" |
        sha256sum --check --status
    mkdir -p "${SDK_ROOT}"
    bsdtar -xf "${platform_archive}" -C "${SDK_ROOT}"
fi

mise exec "java@${JDK_VERSION}" -- \
    "${SDK_MANAGER}" --sdk_root="${SDK_ROOT}" \
    "platforms;android-36" \
    "build-tools;36.0.0" \
    "ndk;${NDK_VERSION}"

rustup target add aarch64-linux-android
if [[ "$(cargo ndk --version 2>/dev/null || true)" != "cargo-ndk 4.1.2" ]]; then
    cargo install cargo-ndk --version 4.1.2 --locked
fi

test "$(sed -n 's/^Pkg.Revision *= *//p' "${SDK_ROOT}/ndk/${NDK_VERSION}/source.properties")" \
    = "${NDK_VERSION}" || {
    printf 'Android NDK revision verification failed\n' >&2
    exit 1
}
test -e "${SDK_ROOT}/platforms/android-36/android.jar" || {
    printf 'Android platform 36 verification failed\n' >&2
    exit 1
}
test -x "${SDK_ROOT}/build-tools/36.0.0/aapt2" || {
    printf 'Android build-tools 36.0.0 verification failed\n' >&2
    exit 1
}
test "$(sed -n 's/^Pkg.Revision *= *//p' "${PLATFORM_TOOLS_PROPERTIES}")" \
    = "${PLATFORM_TOOLS_VERSION}" || {
    printf 'Android platform-tools %s verification failed\n' "${PLATFORM_TOOLS_VERSION}" >&2
    exit 1
}

printf 'Android SDK: %s\n' "${SDK_ROOT}"
mise exec "java@${JDK_VERSION}" -- java -version
"${SDK_ROOT}/platform-tools/adb" version
cargo ndk --version
