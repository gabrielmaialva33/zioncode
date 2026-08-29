#!/usr/bin/env bash
set -Eeuo pipefail

IFS=$'\n\t'
umask 077
export LC_ALL=C

readonly MAX_BOOK_BYTES=16777216
readonly MAX_ATTRIBUTION_BYTES=4096
readonly MAX_PASSPHRASE_FILE_BYTES=1026
readonly MAX_COVER_BYTES=134217728
readonly PHONE_WIDTH=1080
readonly PHONE_HEIGHT=2340
readonly PHONE_RGB_BYTES=7581600
readonly MIN_DEMO_PNG_BYTES=5000000
readonly MAX_DEMO_PNG_BYTES=9000000
readonly BOOK_MEDIA_TYPE='application/vnd.zion.blivre+json'

usage() {
    printf '%s\n' \
        "usage: $0 BOOK.json PASSPHRASE_FILE ATTRIBUTION.txt CLEAN_RGB8.png NEW_OUTPUT_DIR" >&2
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_regular_input() {
    local label="$1"
    local path="$2"
    local maximum_bytes="$3"
    local bytes

    [[ -f "${path}" && ! -L "${path}" && -r "${path}" ]] ||
        die "${label} must be a readable, non-symlink regular file"
    bytes="$(stat -Lc '%s' -- "${path}")" || die "could not stat ${label}"
    [[ "${bytes}" =~ ^[0-9]+$ ]] || die "invalid byte count for ${label}"
    ((bytes > 0 && bytes <= maximum_bytes)) ||
        die "${label} must contain 1..${maximum_bytes} bytes"
}

require_phone_rgb8_png() {
    local label="$1"
    local path="$2"
    local probe

    probe="$(
        ffprobe -v error -select_streams v:0 \
            -show_entries stream=width,height,pix_fmt \
            -of json -- "${path}"
    )" || die "ffprobe could not inspect ${label}"

    jq -e \
        --argjson width "${PHONE_WIDTH}" \
        --argjson height "${PHONE_HEIGHT}" \
        '.streams | length == 1 and
         .[0].width == $width and
         .[0].height == $height and
         .[0].pix_fmt == "rgb24"' \
        <<<"${probe}" >/dev/null ||
        die "${label} must be exactly ${PHONE_WIDTH}x${PHONE_HEIGHT} rgb24"
}

sha256_file() {
    sha256sum -- "$1" | awk '{print $1}'
}

elapsed_ms() {
    awk -v start="$1" -v finish="$2" \
        'BEGIN { printf "%.3f", (finish - start) / 1000000 }'
}

validate_book_json() {
    jq -e '
        type == "object" and
        .schema == 1 and
        (.ordinal | type == "number" and floor == . and . >= 1 and . <= 99) and
        (.id | type == "string" and test("^[A-Z0-9]{3}$")) and
        (.title |
            type == "string" and
            utf8bytelength >= 1 and
            utf8bytelength <= 255 and
            (test("[[:cntrl:]]") | not)) and
        .language == "pt-BR"
    ' "$1" >/dev/null
}

if (($# != 5)); then
    usage
    exit 2
fi

readonly BOOK_INPUT="$1"
readonly PASSPHRASE_FILE="$2"
readonly ATTRIBUTION_INPUT="$3"
readonly COVER_INPUT="$4"
readonly OUTPUT_ARGUMENT="$5"

for dependency in \
    awk basename bash chmod cmp cp date dirname ffmpeg ffprobe iconv id jq mkdir \
    python3 rm rmdir sed sha256sum stat tee; do
    command -v "${dependency}" >/dev/null ||
        die "missing required command: ${dependency}"
done

SCRIPT_DIRECTORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIRECTORY
REPOSITORY_ROOT="$(cd -- "${SCRIPT_DIRECTORY}/.." && pwd -P)"
readonly REPOSITORY_ROOT
ZION_EXECUTABLE="${ZION_BIN:-${REPOSITORY_ROOT}/target/release/zion}"
if [[ "${ZION_EXECUTABLE}" != */* ]]; then
    ZION_EXECUTABLE="$(command -v "${ZION_EXECUTABLE}" || true)"
fi
[[ -n "${ZION_EXECUTABLE}" && -x "${ZION_EXECUTABLE}" && ! -d "${ZION_EXECUTABLE}" ]] ||
    die "zion CLI is not executable; build it with: cargo build --release -p zion-cli"

require_regular_input 'book JSON' "${BOOK_INPUT}" "${MAX_BOOK_BYTES}"
require_regular_input 'passphrase file' "${PASSPHRASE_FILE}" \
    "${MAX_PASSPHRASE_FILE_BYTES}"
require_regular_input 'attribution file' "${ATTRIBUTION_INPUT}" \
    "${MAX_ATTRIBUTION_BYTES}"
require_regular_input 'clean cover PNG' "${COVER_INPUT}" "${MAX_COVER_BYTES}"

CURRENT_UID="$(id -u)"
readonly CURRENT_UID
PASSPHRASE_UID="$(stat -Lc '%u' -- "${PASSPHRASE_FILE}")"
readonly PASSPHRASE_UID
PASSPHRASE_MODE="$(stat -Lc '%a' -- "${PASSPHRASE_FILE}")"
readonly PASSPHRASE_MODE
[[ "${PASSPHRASE_UID}" == "${CURRENT_UID}" ]] ||
    die 'passphrase file must be owned by the current user'
[[ "${PASSPHRASE_MODE}" =~ ^[0-7]{3,4}$ ]] ||
    die 'could not validate passphrase-file permissions'
readonly PASSPHRASE_MODE_VALUE=$((8#${PASSPHRASE_MODE}))
(( (PASSPHRASE_MODE_VALUE & 0077) == 0 )) ||
    die 'passphrase file must not be accessible by group or other users'
(( (PASSPHRASE_MODE_VALUE & 0400) != 0 )) ||
    die 'passphrase file must be readable by its owner'

iconv -f UTF-8 -t UTF-8 -- "${ATTRIBUTION_INPUT}" >/dev/null ||
    die 'attribution file must be valid UTF-8'
require_phone_rgb8_png 'clean cover' "${COVER_INPUT}"

validate_book_json "${BOOK_INPUT}" ||
    die 'book JSON has an unsafe or unsupported schema/id/ordinal/title/language'

ORDINAL="$(jq -er '.ordinal' -- "${BOOK_INPUT}")"
readonly ORDINAL
BOOK_ID="$(jq -er '.id' -- "${BOOK_INPUT}")"
readonly BOOK_ID
BOOK_TITLE="$(jq -er '.title' -- "${BOOK_INPUT}")"
readonly BOOK_TITLE
printf -v ORDINAL_PADDED '%02d' "${ORDINAL}"
readonly ORDINAL_PADDED
readonly ITEM_NAME="${ORDINAL_PADDED} — ${BOOK_TITLE}"
readonly STEM="${ORDINAL_PADDED}-${BOOK_ID}"

OUTPUT_PARENT_ARGUMENT="$(dirname -- "${OUTPUT_ARGUMENT}")"
readonly OUTPUT_PARENT_ARGUMENT
OUTPUT_NAME="$(basename -- "${OUTPUT_ARGUMENT}")"
readonly OUTPUT_NAME
[[ "${OUTPUT_NAME}" != '.' && "${OUTPUT_NAME}" != '..' &&
    "${OUTPUT_NAME}" != *$'\n'* && "${OUTPUT_NAME}" != *$'\r'* ]] ||
    die 'new output directory has an unsafe final component'
[[ -d "${OUTPUT_PARENT_ARGUMENT}" ]] ||
    die 'parent of new output directory must already exist'
OUTPUT_PARENT="$(cd -- "${OUTPUT_PARENT_ARGUMENT}" && pwd -P)"
readonly OUTPUT_PARENT
readonly OUTPUT_DIR="${OUTPUT_PARENT}/${OUTPUT_NAME}"
[[ ! -e "${OUTPUT_DIR}" && ! -L "${OUTPUT_DIR}" ]] ||
    die 'output directory already exists or is a symlink'

readonly BOOK_COPY="${OUTPUT_DIR}/${STEM}.input.json"
readonly ATTRIBUTION_COPY="${OUTPUT_DIR}/${STEM}.attribution.txt"
readonly COVER_COPY="${OUTPUT_DIR}/${STEM}.cover.png"
readonly SEALED_PNG="${OUTPUT_DIR}/${STEM}.arc.png"
readonly REENCODED_PNG="${OUTPUT_DIR}/${STEM}.arc-compression0.png"
readonly OPENED_JSON="${OUTPUT_DIR}/${STEM}.opened.json"
readonly REENCODED_OPENED_JSON="${OUTPUT_DIR}/${STEM}.compression0.opened.json"
readonly METRICS_FILE="${OUTPUT_DIR}/metrics.txt"
readonly TEMP_DIR="${OUTPUT_DIR}/.metrics-work"
readonly CLEAN_RGB="${TEMP_DIR}/clean.rgb"
readonly SEALED_RGB="${TEMP_DIR}/sealed.rgb"
readonly REENCODED_RGB="${TEMP_DIR}/reencoded.rgb"
readonly SEAL_LOG="${TEMP_DIR}/seal.log"
readonly OPEN_LOG="${TEMP_DIR}/open.log"
readonly REOPEN_LOG="${TEMP_DIR}/reopen.log"
readonly CAPACITY_LOG="${TEMP_DIR}/capacity.log"
readonly PSNR_LOG="${TEMP_DIR}/psnr.log"
readonly SSIM_LOG="${TEMP_DIR}/ssim.log"

OUTPUT_CREATED=0
cleanup() {
    local status=$?
    trap - EXIT HUP INT TERM

    for path in \
        "${CLEAN_RGB}" "${SEALED_RGB}" "${REENCODED_RGB}" \
        "${SEAL_LOG}" "${OPEN_LOG}" "${REOPEN_LOG}" \
        "${CAPACITY_LOG}" "${PSNR_LOG}" "${SSIM_LOG}"; do
        if [[ -e "${path}" || -L "${path}" ]]; then
            rm -f -- "${path}" || true
        fi
    done
    if [[ -d "${TEMP_DIR}" && ! -L "${TEMP_DIR}" ]]; then
        rmdir -- "${TEMP_DIR}" || true
    fi

    if ((status != 0 && OUTPUT_CREATED == 1)); then
        for path in \
            "${BOOK_COPY}" "${ATTRIBUTION_COPY}" "${COVER_COPY}" \
            "${SEALED_PNG}" "${REENCODED_PNG}" "${OPENED_JSON}" \
            "${REENCODED_OPENED_JSON}" "${METRICS_FILE}"; do
            if [[ -e "${path}" || -L "${path}" ]]; then
                rm -f -- "${path}" || true
            fi
        done
        if [[ -d "${OUTPUT_DIR}" && ! -L "${OUTPUT_DIR}" ]]; then
            rmdir -- "${OUTPUT_DIR}" ||
                printf 'warning: incomplete private output remains at %s\n' \
                    "${OUTPUT_DIR}" >&2
        fi
    fi
    exit "${status}"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -m 0700 -- "${OUTPUT_DIR}" ||
    die 'could not atomically create the new private output directory'
OUTPUT_CREATED=1
mkdir -m 0700 -- "${TEMP_DIR}"

cp -- "${BOOK_INPUT}" "${BOOK_COPY}"
cp -- "${ATTRIBUTION_INPUT}" "${ATTRIBUTION_COPY}"
cp -- "${COVER_INPUT}" "${COVER_COPY}"
chmod 0600 -- "${BOOK_COPY}" "${ATTRIBUTION_COPY}" "${COVER_COPY}"

[[ "$(sha256_file "${BOOK_INPUT}")" == "$(sha256_file "${BOOK_COPY}")" ]] ||
    die 'book JSON changed while it was being snapshotted'
[[ "$(sha256_file "${ATTRIBUTION_INPUT}")" == "$(sha256_file "${ATTRIBUTION_COPY}")" ]] ||
    die 'attribution changed while it was being snapshotted'
[[ "$(sha256_file "${COVER_INPUT}")" == "$(sha256_file "${COVER_COPY}")" ]] ||
    die 'cover changed while it was being snapshotted'
validate_book_json "${BOOK_COPY}" ||
    die 'snapshotted book JSON failed structural validation'
SNAPSHOT_ORDINAL="$(jq -er '.ordinal' -- "${BOOK_COPY}")"
SNAPSHOT_BOOK_ID="$(jq -er '.id' -- "${BOOK_COPY}")"
SNAPSHOT_BOOK_TITLE="$(jq -er '.title' -- "${BOOK_COPY}")"
SNAPSHOT_LANGUAGE="$(jq -er '.language' -- "${BOOK_COPY}")"
readonly SNAPSHOT_ORDINAL SNAPSHOT_BOOK_ID SNAPSHOT_BOOK_TITLE SNAPSHOT_LANGUAGE
[[ "${SNAPSHOT_ORDINAL}" == "${ORDINAL}" &&
    "${SNAPSHOT_BOOK_ID}" == "${BOOK_ID}" &&
    "${SNAPSHOT_BOOK_TITLE}" == "${BOOK_TITLE}" &&
    "${SNAPSHOT_LANGUAGE}" == 'pt-BR' ]] ||
    die 'book identity changed while it was being snapshotted'
require_phone_rgb8_png 'snapshotted clean cover' "${COVER_COPY}"

"${ZION_EXECUTABLE}" arc capacity \
    --width "${PHONE_WIDTH}" \
    --height "${PHONE_HEIGHT}" \
    --profile safe >"${CAPACITY_LOG}"

seal_start="$(date +%s%N)"
if ! "${ZION_EXECUTABLE}" arc seal \
    "${BOOK_COPY}" \
    "${COVER_COPY}" \
    --output "${SEALED_PNG}" \
    --name "${ITEM_NAME}" \
    --media-type "${BOOK_MEDIA_TYPE}" \
    --attribution-file "${ATTRIBUTION_COPY}" \
    --profile safe \
    --passphrase-file "${PASSPHRASE_FILE}" 2>"${SEAL_LOG}"; then
    sed -n '1,20p' "${SEAL_LOG}" >&2
    die 'ARC sealing failed'
fi
seal_finish="$(date +%s%N)"
sed -n '1,20p' "${SEAL_LOG}" >&2

open_start="$(date +%s%N)"
if ! "${ZION_EXECUTABLE}" arc open \
    "${SEALED_PNG}" \
    --output "${OPENED_JSON}" \
    --passphrase-file "${PASSPHRASE_FILE}" 2>"${OPEN_LOG}"; then
    sed -n '1,20p' "${OPEN_LOG}" >&2
    die 'opening the first ARC PNG failed'
fi
open_finish="$(date +%s%N)"
sed -n '1,20p' "${OPEN_LOG}" >&2
cmp -s -- "${BOOK_COPY}" "${OPENED_JSON}" ||
    die 'first ARC open did not reproduce the exact book JSON'

ffmpeg -n -hide_banner -nostdin -loglevel error \
    -i "${SEALED_PNG}" \
    -map_metadata -1 \
    -frames:v 1 \
    -pix_fmt rgb24 \
    -compression_level 0 \
    "${REENCODED_PNG}"

REENCODED_PNG_BYTES="$(stat -Lc '%s' -- "${REENCODED_PNG}")"
readonly REENCODED_PNG_BYTES
((REENCODED_PNG_BYTES >= MIN_DEMO_PNG_BYTES &&
    REENCODED_PNG_BYTES <= MAX_DEMO_PNG_BYTES)) ||
    die "compression-level-0 PNG is outside the 5,000,000..9,000,000 byte demo band"
require_phone_rgb8_png 'compression-level-0 ARC PNG' "${REENCODED_PNG}"

reopen_start="$(date +%s%N)"
if ! "${ZION_EXECUTABLE}" arc open \
    "${REENCODED_PNG}" \
    --output "${REENCODED_OPENED_JSON}" \
    --passphrase-file "${PASSPHRASE_FILE}" 2>"${REOPEN_LOG}"; then
    sed -n '1,20p' "${REOPEN_LOG}" >&2
    die 'opening the losslessly re-encoded ARC PNG failed'
fi
reopen_finish="$(date +%s%N)"
sed -n '1,20p' "${REOPEN_LOG}" >&2
cmp -s -- "${BOOK_COPY}" "${REENCODED_OPENED_JSON}" ||
    die 'losslessly re-encoded ARC open did not reproduce the exact book JSON'

decode_rgb8() {
    local png_path="$1"
    local rgb_path="$2"

    ffmpeg -n -hide_banner -nostdin -loglevel error \
        -i "${png_path}" -frames:v 1 -f rawvideo -pix_fmt rgb24 "${rgb_path}"
    [[ "$(stat -Lc '%s' -- "${rgb_path}")" == "${PHONE_RGB_BYTES}" ]] ||
        die 'decoded raw RGB byte count is not the expected phone raster size'
}

decode_rgb8 "${COVER_COPY}" "${CLEAN_RGB}"
decode_rgb8 "${SEALED_PNG}" "${SEALED_RGB}"
decode_rgb8 "${REENCODED_PNG}" "${REENCODED_RGB}"

cmp -s -- "${SEALED_RGB}" "${REENCODED_RGB}" ||
    die 'lossless PNG re-encode changed one or more RGB8 samples'
SEALED_RGB_SHA256="$(sha256_file "${SEALED_RGB}")"
readonly SEALED_RGB_SHA256
REENCODED_RGB_SHA256="$(sha256_file "${REENCODED_RGB}")"
readonly REENCODED_RGB_SHA256
[[ "${SEALED_RGB_SHA256}" == "${REENCODED_RGB_SHA256}" ]] ||
    die 'lossless PNG re-encode raw-RGB hashes differ'

CLEAN_BOOTSTRAP_MAGIC="$(
    python3 - "${CLEAN_RGB}" <<'PY'
import pathlib
import sys

channels = pathlib.Path(sys.argv[1]).read_bytes()[:32]
if len(channels) != 32:
    raise SystemExit("short RGB fixture")
magic = bytes(
    sum((channels[byte * 8 + bit] & 1) << (7 - bit) for bit in range(8))
    for byte in range(4)
)
print(magic.hex())
PY
)"
readonly CLEAN_BOOTSTRAP_MAGIC
[[ "${CLEAN_BOOTSTRAP_MAGIC}" != '5a415243' ]] ||
    die 'clean cover already contains the public ARC ZARC bootstrap magic'

IFS=' ' read -r \
    CHANGED_CHANNELS TOTAL_CHANNELS CHANGED_PERCENT MEAN_ABSOLUTE_DELTA MAX_DELTA < <(
    python3 - "${CLEAN_RGB}" "${SEALED_RGB}" <<'PY'
import pathlib
import sys

left_path = pathlib.Path(sys.argv[1])
right_path = pathlib.Path(sys.argv[2])
changed = 0
delta_sum = 0
maximum = 0
total = 0
with left_path.open("rb") as left, right_path.open("rb") as right:
    while True:
        left_chunk = left.read(1024 * 1024)
        right_chunk = right.read(1024 * 1024)
        if len(left_chunk) != len(right_chunk):
            raise SystemExit("RGB lengths differ")
        if not left_chunk:
            break
        for before, after in zip(left_chunk, right_chunk, strict=True):
            delta = abs(before - after)
            changed += delta != 0
            delta_sum += delta
            maximum = max(maximum, delta)
        total += len(left_chunk)
if total == 0:
    raise SystemExit("empty RGB fixture")
print(
    changed,
    total,
    f"{changed * 100.0 / total:.9f}",
    f"{delta_sum / total:.12f}",
    maximum,
)
PY
)
[[ "${TOTAL_CHANNELS}" == "${PHONE_RGB_BYTES}" && "${MAX_DELTA}" == '1' ]] ||
    die 'sealed raster violates ARC minimal channel-delta expectations'

ffmpeg -hide_banner -nostdin -loglevel info \
    -i "${COVER_COPY}" -i "${SEALED_PNG}" \
    -filter_complex '[0:v][1:v]psnr' -frames:v 1 -f null - \
    >/dev/null 2>"${PSNR_LOG}"
PSNR_DB="$(
    awk '
        /PSNR/ {
            for (field = 1; field <= NF; field++) {
                if ($field ~ /^average:/) {
                    sub(/^average:/, "", $field)
                    value = $field
                }
            }
        }
        END { if (value == "") exit 1; print value }
    ' "${PSNR_LOG}"
)" || die 'could not parse ffmpeg PSNR output'
readonly PSNR_DB

ffmpeg -hide_banner -nostdin -loglevel info \
    -i "${COVER_COPY}" -i "${SEALED_PNG}" \
    -filter_complex '[0:v][1:v]ssim' -frames:v 1 -f null - \
    >/dev/null 2>"${SSIM_LOG}"
SSIM="$(
    awk '
        /SSIM/ {
            for (field = 1; field <= NF; field++) {
                if ($field ~ /^All:/) {
                    sub(/^All:/, "", $field)
                    value = $field
                }
            }
        }
        END { if (value == "") exit 1; print value }
    ' "${SSIM_LOG}"
)" || die 'could not parse ffmpeg SSIM output'
readonly SSIM

CODEWORDS="$(
    sed -n 's/.*codewords=\([0-9][0-9]*\).*/\1/p' "${SEAL_LOG}"
)"
readonly CODEWORDS
CIPHERTEXT_BYTES="$(
    sed -n 's/.*ciphertext_bytes=\([0-9][0-9]*\).*/\1/p' "${SEAL_LOG}"
)"
readonly CIPHERTEXT_BYTES
MAX_CODEWORDS="$(
    sed -n 's/^max_codewords[[:space:]]*:[[:space:]]*\([0-9][0-9]*\)$/\1/p' \
        "${CAPACITY_LOG}"
)"
readonly MAX_CODEWORDS
MAX_CIPHERTEXT_BYTES="$(
    sed -n 's/^max_ciphertext_bytes[[:space:]]*:[[:space:]]*\([0-9][0-9]*\)$/\1/p' \
        "${CAPACITY_LOG}"
)"
readonly MAX_CIPHERTEXT_BYTES
[[ "${CODEWORDS}" =~ ^[0-9]+$ && "${CIPHERTEXT_BYTES}" =~ ^[0-9]+$ &&
    "${MAX_CODEWORDS}" =~ ^[0-9]+$ && "${MAX_CIPHERTEXT_BYTES}" =~ ^[0-9]+$ ]] ||
    die 'could not parse ARC capacity/seal metrics'

readonly INTERLEAVED_BYTES=$((CODEWORDS * 255))
readonly RS_PARITY_BYTES=$((INTERLEAVED_BYTES - CIPHERTEXT_BYTES))
readonly REQUIRED_PAYLOAD_BITS=$((INTERLEAVED_BYTES * 8))
BOOK_BYTES="$(stat -Lc '%s' -- "${BOOK_COPY}")"
readonly BOOK_BYTES
ATTRIBUTION_BYTES="$(stat -Lc '%s' -- "${ATTRIBUTION_COPY}")"
readonly ATTRIBUTION_BYTES
COVER_PNG_BYTES="$(stat -Lc '%s' -- "${COVER_COPY}")"
readonly COVER_PNG_BYTES
SEALED_PNG_BYTES="$(stat -Lc '%s' -- "${SEALED_PNG}")"
readonly SEALED_PNG_BYTES
BOOK_SHA256="$(sha256_file "${BOOK_COPY}")"
readonly BOOK_SHA256
OPENED_SHA256="$(sha256_file "${OPENED_JSON}")"
readonly OPENED_SHA256
REENCODED_OPENED_SHA256="$(sha256_file "${REENCODED_OPENED_JSON}")"
readonly REENCODED_OPENED_SHA256
COVER_PNG_SHA256="$(sha256_file "${COVER_COPY}")"
readonly COVER_PNG_SHA256
SEALED_PNG_SHA256="$(sha256_file "${SEALED_PNG}")"
readonly SEALED_PNG_SHA256
REENCODED_PNG_SHA256="$(sha256_file "${REENCODED_PNG}")"
readonly REENCODED_PNG_SHA256
CLEAN_RGB_SHA256="$(sha256_file "${CLEAN_RGB}")"
readonly CLEAN_RGB_SHA256
SEAL_WALL_MS="$(elapsed_ms "${seal_start}" "${seal_finish}")"
readonly SEAL_WALL_MS
OPEN_WALL_MS="$(elapsed_ms "${open_start}" "${open_finish}")"
readonly OPEN_WALL_MS
REOPEN_WALL_MS="$(elapsed_ms "${reopen_start}" "${reopen_finish}")"
readonly REOPEN_WALL_MS

{
    printf 'zion_arc_art_demo=1\n'
    printf 'item_id=%s\n' "${BOOK_ID}"
    printf 'item_name=%s\n' "${ITEM_NAME}"
    printf 'dimensions=%sx%s\n' "${PHONE_WIDTH}" "${PHONE_HEIGHT}"
    printf 'pixel_format=rgb24\n'
    printf 'clean_bootstrap_magic_hex=%s\n' "${CLEAN_BOOTSTRAP_MAGIC}"
    printf 'clean_cover_status=no_ZARC_magic_before_seal\n'
    printf 'book_bytes=%s\n' "${BOOK_BYTES}"
    printf 'attribution_bytes=%s\n' "${ATTRIBUTION_BYTES}"
    printf 'cover_png_bytes=%s\n' "${COVER_PNG_BYTES}"
    printf 'sealed_png_bytes=%s\n' "${SEALED_PNG_BYTES}"
    printf 'compression0_png_bytes=%s\n' "${REENCODED_PNG_BYTES}"
    printf 'max_codewords=%s\n' "${MAX_CODEWORDS}"
    printf 'max_ciphertext_bytes=%s\n' "${MAX_CIPHERTEXT_BYTES}"
    printf 'used_codewords=%s\n' "${CODEWORDS}"
    printf 'ciphertext_bytes=%s\n' "${CIPHERTEXT_BYTES}"
    printf 'interleaved_bytes=%s\n' "${INTERLEAVED_BYTES}"
    printf 'rs_parity_bytes=%s\n' "${RS_PARITY_BYTES}"
    printf 'required_payload_bits=%s\n' "${REQUIRED_PAYLOAD_BITS}"
    printf 'changed_channels=%s\n' "${CHANGED_CHANNELS}"
    printf 'changed_channels_percent=%s\n' "${CHANGED_PERCENT}"
    printf 'mean_absolute_channel_delta=%s\n' "${MEAN_ABSOLUTE_DELTA}"
    printf 'maximum_channel_delta=%s\n' "${MAX_DELTA}"
    printf 'psnr_db=%s\n' "${PSNR_DB}"
    printf 'ssim=%s\n' "${SSIM}"
    printf 'seal_wall_ms=%s\n' "${SEAL_WALL_MS}"
    printf 'open_wall_ms=%s\n' "${OPEN_WALL_MS}"
    printf 'compression0_open_wall_ms=%s\n' "${REOPEN_WALL_MS}"
    printf 'book_sha256=%s\n' "${BOOK_SHA256}"
    printf 'opened_sha256=%s\n' "${OPENED_SHA256}"
    printf 'compression0_opened_sha256=%s\n' "${REENCODED_OPENED_SHA256}"
    printf 'cover_png_sha256=%s\n' "${COVER_PNG_SHA256}"
    printf 'sealed_png_sha256=%s\n' "${SEALED_PNG_SHA256}"
    printf 'compression0_png_sha256=%s\n' "${REENCODED_PNG_SHA256}"
    printf 'clean_rgb_sha256=%s\n' "${CLEAN_RGB_SHA256}"
    printf 'sealed_rgb_sha256=%s\n' "${SEALED_RGB_SHA256}"
    printf 'compression0_rgb_sha256=%s\n' "${REENCODED_RGB_SHA256}"
    printf 'first_open_cmp=exact\n'
    printf 'compression0_open_cmp=exact\n'
    printf 'sealed_to_compression0_rgb_identity=exact\n'
    printf 'argon2_time=not_separately_instrumented_in_cli\n'
} | tee "${METRICS_FILE}"

chmod 0600 -- \
    "${SEALED_PNG}" "${REENCODED_PNG}" "${OPENED_JSON}" \
    "${REENCODED_OPENED_JSON}" "${METRICS_FILE}"

rm -f -- \
    "${CLEAN_RGB}" "${SEALED_RGB}" "${REENCODED_RGB}" \
    "${SEAL_LOG}" "${OPEN_LOG}" "${REOPEN_LOG}" \
    "${CAPACITY_LOG}" "${PSNR_LOG}" "${SSIM_LOG}"
rmdir -- "${TEMP_DIR}"

OUTPUT_CREATED=0
trap - EXIT HUP INT TERM
printf 'demo output: %s\n' "${OUTPUT_DIR}"
