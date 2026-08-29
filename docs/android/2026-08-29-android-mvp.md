# Zion ARC Android MVP

Date: 2026-08-29

Status: the host build, Rust smoke tests, Android unit tests, lint, debug APK,
and unsigned release APK pass. No physical Android device or emulator was
available for this run, so device behavior and performance remain explicitly
pending.

This MVP opens one original ARC PNG, authenticates and recovers one normalized
book through the existing Rust implementation, renders its chapters and marker
stream in a Portuguese Compose UI, and exports the recovered `raw_usfm`. It does
not embed a Bible corpus, a carrier, a passphrase, or a generated capsule.

## Architecture

The byte path is intentionally narrow:

```text
OpenDocument("image/png")
  -> ContentResolver InputStream (closed with `use`)
  -> bounded original PNG ByteArray (maximum 128 MiB, with a one-byte probe)
  -> Dispatchers.Default
  -> direct JNI (`jni` 0.22.4)
  -> JByteArray length checks before either native copy
  -> zion_arc::open_png_bytes
  -> immediately zeroized recovered-content allocation
  -> media-type check + deserialize + structural validation
  -> canonical reserialization through zion_corpus::BookDocument
  -> validated, zeroized JSON ByteArray
  -> immutable UI model
  -> LazyColumn reader / CreateDocument raw USFM export
```

No `Bitmap`, pixel decoder, PNG encoder, Kotlin cryptography, subprocess,
database, network client, analytics SDK, or persistence layer is in the app.
The input passed to ARC is the exact byte stream returned by the selected
document provider. Stream I/O runs on `Dispatchers.IO`; native recovery and JSON
model construction run on `Dispatchers.Default`.

`zion-android` builds as both `cdylib` and `rlib`. Its pure
`decode_book_png` function and internal validation/reserialization path are
host-testable, while the single JNI method is exported as
`Java_org_zioncode_app_NativeBridge_decodeBookPng`. The bridge uses
`EnvUnowned::with_env`, whose pinned `jni` 0.22.4 implementation catches panics
at the native boundary, and a custom error policy that emits only fixed error
codes. It never formats an underlying ARC, JSON, JNI, or panic error for the UI.

Every failure after `zion_arc::open_png_bytes` succeeds maps to the same
`RecoveryFailed` / `ZION_RECOVERY_FAILED` contract used for a wrong password.
That includes an unexpected authenticated media type, JSON deserialization,
book-structure validation, canonical reserialization, and the defensive Kotlin
JSON parser boundary. The UI therefore exposes no post-KDF validity oracle.
Pre-KDF malformed-image and passphrase-length errors remain separate and contain
no recovered data.

## Validated toolchain

The setup script installs developer tools outside the repository and does not
activate a project or global Java version:

```bash
./scripts/setup-android.sh
```

`ANDROID_SDK_ROOT` can override the default SDK directory. The live validated
installation used these sanitized paths and pins:

| Component | Exact version | Live path or source |
|---|---|---|
| JDK for Android builds | Eclipse Temurin 17.0.20.1 | `~/.local/share/mise/installs/java/temurin-17.0.20+101` |
| Android command-line tools | 22.0, archive build 15859902 | `~/.local/share/android-sdk/cmdline-tools/latest` |
| Android platform | API 36, package revision 2 | `~/.local/share/android-sdk/platforms/android-36` |
| Android build tools | 36.0.0 | `~/.local/share/android-sdk/build-tools/36.0.0` |
| Platform tools / adb | 37.0.1-15733141 | pinned `platform-tools_r37.0.1-linux.zip` |
| Android NDK | 28.2.13676358 (r28c) | `~/.local/share/android-sdk/ndk/28.2.13676358` |
| Gradle wrapper | 9.1.0 | `android/gradle/wrapper` |
| Android Gradle Plugin | 9.0.1 | `android/build.gradle.kts` |
| Compose compiler plugin | 2.2.10 | `android/build.gradle.kts` |
| Rust | 1.95.0 | pinned workspace toolchain |
| Rust Android target | `aarch64-linux-android` | rustup component |
| cargo-ndk | exactly 4.1.2 | Cargo-installed binary |
| jni crate | exactly 0.22.4 | workspace lockfile |

Before and after setup, `mise current java` remained
`temurin-25.0.4+7.0.LTS`; JDK 17 was invoked only through `mise exec`. No shell
profile, active Java selection, display profile, or VRR setting was changed.
The live root filesystem had approximately 1.5 TiB free before installation.

The command-line-tools archive was verified before extraction with the SHA-256
published on the Android Studio download page:

```text
4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583
```

Platform-tools no longer uses sdkmanager's moving `platform-tools` coordinate.
The setup script downloads Google's exact versioned 37.0.1 Linux archive,
checks its 9,054,187-byte length and the SHA-1 published in Google's official
SDK repository metadata, then also checks the pinned SHA-256 of those verified
bytes:

```text
official repository SHA-1: 477254aa5f903c15cf51001717bdf347fb6b53e0
pinned SHA-256:            d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1
```

The Gradle wrapper pins and verifies the official 9.1.0 binary distribution:

```text
gradle-9.1.0-bin.zip SHA-256:
a17ddd85a26b6a7f5ddb71ff8b05fc5104c0202c6e64782429790c933686c806

gradle-9.1.0-wrapper.jar SHA-256:
76805e32c009c0cf0dd5d206bddc9fb22ea42e84db904b764f3047de095493f3
```

Command-line tools 22.0 currently prints a deprecation notice for
`sdkmanager` and points to the newer `android sdk` command. The setup retains
`sdkmanager` because it is still the official documented headless package
installer and the hackathon matrix requires exact package coordinates. The
script verifies the installed command-line-tools revision, NDK revision,
platform JAR, build-tools executable, Rust target, and cargo-ndk version after
installation.

## Build

Run from the repository root:

```bash
./scripts/build-android.sh
```

The script:

1. selects only NDK 28.2.13676358;
2. removes only the exact generated
   `app/src/main/jniLibs/arm64-v8a/libzion_android.so`, never the surrounding
   `jniLibs` tree;
3. cross-builds `zion-android` for Android API 26 and `arm64-v8a` with
   cargo-ndk 4.1.2;
4. requires a fresh regular nonempty library under the conventional
   `app/src/main/jniLibs/arm64-v8a` directory;
5. refuses any unexpected generated `.so` beside `libzion_android.so`; and
6. invokes Gradle with isolated JDK 17 and strict dependency verification to
   run unit tests, lint, debug assembly, and release assembly.

Generated native libraries and Android outputs are ignored. They must not be
committed:

```text
android/app/src/main/jniLibs/arm64-v8a/libzion_android.so
android/app/build/outputs/apk/debug/app-debug.apk
android/app/build/outputs/apk/release/app-release-unsigned.apk
```

The release APK is deliberately unsigned. A release signing identity is an
operator/deployment concern and is not stored in this repository. The debug APK
uses Gradle's local debug signing and is the installable hackathon artifact.

## Demo flow

1. Produce an ARC book PNG using the pinned desktop CLI flow in
   [`docs/hackathon/2026-08-29-cli-demo.md`](../hackathon/2026-08-29-cli-demo.md).
2. Transfer the original `.arc.png` file to the phone. Do not send a screenshot,
   JPEG conversion, resized copy, social-media rendition, print, or camera photo.
3. Install the debug APK on an arm64 Android 8.0+ device.
4. Tap **Selecionar arte PNG**. The system document picker is filtered to the
   single MIME type `image/png` and grants access only to the selected URI.
5. Enter the passphrase and tap **Abrir offline**. The password field is cleared
   as soon as the attempt begins.
6. Read the title, source, chapters, verses, and preserved marker records in the
   `LazyColumn` reader.
7. Tap **Exportar USFM**, choose a destination in the system
   `CreateDocument("text/plain")` picker, and compare the output with the
   original source USFM. The synthetic Rust smoke test proves byte-identical
   UTF-8 export material for its generated fixture.
8. Open **Sobre** to show the exact BLIVRE authors, version, attribution, license
   links, ARC limits, and privacy caveats.

The app requests no broad storage permission. A document provider selected in
the system picker may itself be backed by a cloud service; that provider is
outside the app's process and permission set. Likewise, tapping an attribution
link delegates the URL to another installed app. Zion ARC itself has no
`INTERNET` permission.

## Security and limitations

- The Rust bridge wraps the recovered content and JNI passphrase copy with
  `Zeroizing` immediately after ARC opens, before inspecting authenticated
  metadata. An owned-document guard clears every `BookDocument` string,
  including nested marker text, source fields, and authors, on all validation
  and serialization exits. Kotlin clears the selected PNG bytes, native JSON
  bytes, UTF-8 passphrase bytes, and temporary USFM export bytes in `finally`
  blocks.
- Compose password state and decoded `String` values are immutable JVM objects.
  Their internal copies cannot be reliably zeroized. Clearing state only drops
  references and leaves reclamation to the runtime; the About screen states this
  limitation directly. The password field uses `KeyboardType.Password`, disables
  autocorrect, and rejects candidate state above 1,024 UTF-8 bytes; this reduces
  exposure but cannot control copies retained by an IME or the JVM.
- Decoded text, passphrases, ARC errors, and native error details are never
  logged. The app contains no logging, crash-reporting, analytics, backup, or
  persistence integration. Android backup is disabled.
- The bounded reader allows at most 134,217,728 PNG bytes and reads at most one
  additional byte to detect overflow. Before JNI copies either Java array,
  `JByteArray::len` independently enforces the same PNG maximum and a passphrase
  length of 1..=1,024 bytes. ARC's own input bounds remain authoritative after
  the copy. Both input and output streams are closed.
- ARC v1 is for exact lossless RGB8 pixel samples. It does not promise survival
  through screenshots, JPEG, resize, crop, print/camera capture, color
  correction, or social-media processing.
- ZARC magic, public identifiers, dimensions, and density remain detectable and
  correlatable. This is not a claim of forensic invisibility.
- The cover appearance and ancillary PNG metadata are not authenticated. ARC is
  not an independently audited or formally verified cryptographic system.
- Transporting content can be constrained by local law or inspection policy.
  The software does not provide legal authorization or advice.
- The app opens exactly one book at a time. It intentionally has no library,
  search index, database, background service, model inference, or network sync.
- Standard Gradle SHA-256 dependency-verification metadata is checked in at
  `android/gradle/verification-metadata.xml`, with metadata verification enabled,
  no trusted/ignored artifact exceptions, and Gradle's default strict mode. It
  was generated from the exact unit-test, lint, debug, and release task set and
  then passed a clean offline strict rebuild of all 96 tasks. This freezes the
  resolved bytes but is not an independent provenance audit of all 322
  components. Platform-tools is independently pinned to the exact versioned
  archive described above instead of sdkmanager's moving coordinate.

The complete format threat model and limits remain normative in
[`docs/specs/2026-08-29-zion-arc-v1.md`](../specs/2026-08-29-zion-arc-v1.md).

## Host validation record

All results below were measured on 2026-08-29 from the dirty Phase 2/3 workspace
without removing or overwriting unrelated work.

| Command | Result |
|---|---|
| `./scripts/setup-android.sh` | PASS; exact packages already installed and reverified |
| `bash -n scripts/setup-android.sh scripts/build-android.sh` | PASS |
| `shellcheck scripts/setup-android.sh scripts/build-android.sh` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --workspace` | PASS; includes seven `zion-android` roundtrip/contract/oracle/boundary/zeroization tests and the frozen ARC vector |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| cargo-ndk arm64 release build | PASS twice from separate fresh target directories; 6.05 s and 6.24 s, both including `zstd-sys` |
| `:app:testDebugUnitTest` | PASS; eight bounded-stream, UTF-8 password-limit, shared-contract, and generic parser-error tests |
| `:app:lint` | PASS with warnings treated as errors |
| `:app:assembleDebug` | PASS |
| `:app:assembleRelease` | PASS; unsigned release |
| clean complete `build-android.sh` run | PASS; fresh Rust target plus Gradle 21 s, 95 tasks |
| second equivalent no-cache Gradle run | PASS; `--no-build-cache --rerun-tasks`, 22 s, all 95 tasks executed |
| strict dependency verification | PASS; clean offline no-cache rebuild, all 96 tasks executed |

Lint intentionally disables only `AndroidGradlePluginVersion`,
`GradleDependency`, and `ChromeOsAbiSupport`: the first two conflict with the
explicitly frozen AGP/API 36 matrix and the last conflicts with the required
arm64-only MVP. All other warnings are errors. AGP also emits a non-fatal SDK XML
version warning when paired with the newer command-line-tools 22.0 package; all
test, lint, resource, R8, and packaging tasks complete successfully.

The shared `testdata/contracts/book_document_v1.json` fixture contains no
religious or BLIVRE text. Rust deserializes it as the real
`zion_corpus::BookDocument`, validates it through the production mobile
structure checks, compares canonical serde serialization, and asserts two
chapters, ordered verse/paragraph/non-verse and inline opening/closing markers,
plus byte-exact BOM/CRLF `raw_usfm`. Android's JVM test consumes that same file
and asserts the exact projected reader-row order and raw export string. The Rust
ARC smoke path seals the contract with a synthetic RGB carrier and test-only
passphrase entirely in memory, opens through the pure mobile decoder, and
compares the validated JSON. Authenticated malformed JSON, wrong media type,
invalid book structure, and a wrong password all assert the identical production
variant, message, and JNI code: `RecoveryFailed`, `capsule recovery failed`, and
`ZION_RECOVERY_FAILED`. Boundary tests cover zero, exact maximum, and maximum
plus one before JNI copying; a separate test covers every document string cleared
by the native zeroizer.

## Final artifact inspection

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| debug APK | 12,790,739 | `4e5e210fb093a31b4caf1e7f1733f623c492a459285c0e0bed15fc2254ca0c0e` |
| unsigned release APK | 2,199,346 | `fbc5a9ced23ddb015d4a0646b634ebefd3e8c210ca77255df046e93a582a7788` |
| cargo-ndk output `libzion_android.so` | 1,489,576 | `b4d67d8ea666c2d934ccbe5ad93c23dbd916ce42d7bbe9e5031be32688ba8d21` |
| packaged stripped `libzion_android.so` | 1,083,472 | `5b480233c5ebf2ffb5719992bdef2a528217eb7964180bdf057bb4e2243270d7` |

These four sizes and hashes were identical across two separate fresh Cargo
target directories and two equivalent clean Android assemblies on this host;
the second Gradle build disabled the build cache and reran all tasks. This is an
observed same-host result for the pinned inputs, not a cross-host reproducible
build guarantee.

`file` identifies the cargo-ndk output as an ELF64 ARM aarch64 shared object for
Android API 26, built by NDK r28c. `llvm-nm` finds exactly the intended JNI
export. The APK native entries are:

```text
lib/arm64-v8a/libandroidx.graphics.path.so   10,096 bytes
lib/arm64-v8a/libzion_android.so          1,083,472 bytes
```

There is exactly one packaged `libzion_android.so`, and every native entry is
under `arm64-v8a`; there is no x86, x86_64, or 32-bit ARM directory. The small
second library is the transitive AndroidX Compose `graphics-path` runtime, not a
second project ABI or Rust library.

`apkanalyzer` reports:

```text
application id: org.zioncode.app
min SDK:        26
target SDK:     36
debuggable:     false (release)
INTERNET or broad storage/media permissions: 0
```

The merged manifest contains only AndroidX's app-scoped signature permission
`org.zioncode.app.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION`; it declares no
Android platform permission. `allowBackup` and cleartext traffic are false. The
APK file list contains no corpus, book JSON, USFM, upstream ZIP, passphrase,
carrier, or ARC PNG. Its only `assets/` entries are Compose/AndroidX baseline
profiles.

`git check-ignore` confirms the `.so` and both APKs are ignored, and a complete
untracked-file check found no APK, shared object, passphrase, corpus archive,
opened plaintext, or generated ARC image eligible for commit.

## Pending device validation

The sanitized check returned `connected_device_count=0`. No emulator package or
system image is installed either. Consequently, this report does **not** claim
APK installation, JNI loading on ART, real picker-provider behavior, rendering,
export, airplane-mode use, memory pressure, or timing on a Galaxy A57 or any
other phone.

Before a hackathon demo, run these physical-device checks on an arm64 device:

1. install the debug APK and confirm the package launches on the minimum and a
   target-class Android version;
2. open generated Genesis, Psalms, and Revelation capsules with their separately
   transferred passphrase;
3. compare each exported USFM byte-for-byte with its pinned source;
4. repeat with airplane mode enabled;
5. exercise wrong-password, non-ARC PNG, truncated PNG, and 128 MiB plus one byte
   cases;
6. measure cold and warm recovery latency and peak memory, especially Psalms;
7. rotate/background/restore during recovery and verify cancellation and state;
8. inspect the UI with large fonts and TalkBack; and
9. verify that screenshot/JPEG/resized inputs fail without producing plaintext.

Do not commit any device test corpus, passphrase, `.so`, or APK after these
checks.

## Official implementation references

Only primary vendor/project documentation was used for the Android toolchain and
native boundary:

- Android Studio downloads and command-line-tools checksums:
  <https://developer.android.com/studio>
- `sdkmanager` installation and exact package coordinates:
  <https://developer.android.com/tools/sdkmanager>
- Android command-line tool inventory:
  <https://developer.android.com/tools>
- AGP 9.0.1 compatibility matrix (Gradle 9.1.0, build tools 36.0.0,
  NDK 28.2.13676358, JDK 17, API 36.1 maximum):
  <https://developer.android.com/build/releases/agp-9-0-0-release-notes>
- AGP 9 built-in Kotlin migration:
  <https://developer.android.com/build/migrate-to-built-in-kotlin>
- Compose setup and BOM guidance:
  <https://developer.android.com/develop/ui/compose/setup> and
  <https://developer.android.com/develop/ui/compose/bom>
- `ActivityResultContracts.OpenDocument` API:
  <https://developer.android.com/reference/androidx/activity/result/contract/ActivityResultContracts.OpenDocument>
- Storage Access Framework and `ACTION_CREATE_DOCUMENT` behavior:
  <https://developer.android.com/training/data-storage/shared/documents-files>
- Android NDK installation and version selection:
  <https://developer.android.com/studio/projects/install-ndk>
- Native library ABI packaging:
  <https://developer.android.com/ndk/guides/abis>
- Gradle 9.1.0 release and wrapper verification:
  <https://docs.gradle.org/9.1.0/release-notes.html>,
  <https://docs.gradle.org/9.1.0/userguide/gradle_wrapper.html>, and
  <https://gradle.org/release-checksums/>.
- Gradle 9.1 dependency verification and strict-mode metadata:
  <https://docs.gradle.org/9.1.0/userguide/dependency_verification.html>.
- Google's versioned SDK package URL and published platform-tools checksum:
  <https://dl.google.com/android/repository/repository2-3.xml>.
- Mise isolated execution and Java support:
  <https://mise.jdx.dev/getting-started.html> and
  <https://mise.jdx.dev/lang/java.html>
- Rust Android platform target:
  <https://doc.rust-lang.org/rustc/platform-support/android.html>
- cargo-ndk 4.1.2 release and official usage:
  <https://github.com/bbqsrc/cargo-ndk/releases/tag/v4.1.2> and
  <https://github.com/bbqsrc/cargo-ndk/blob/v4.1.2/README.md>
- `jni` 0.22.4 `EnvUnowned::with_env` panic boundary and API:
  <https://docs.rs/jni/0.22.4/jni/struct.EnvUnowned.html#method.with_env> and
  <https://github.com/jni-rs/jni-rs/blob/v0.22.4/crates/jni/docs/0.22-MIGRATION.md>
