#!/usr/bin/env bash
set -Eeuo pipefail

IFS=$'\n\t'
umask 077
export LC_ALL=C

readonly MAX_BOOK_BYTES=16777216
readonly MAX_MANIFEST_BYTES=1048576
readonly MAX_CANONICAL_MAP_BYTES=1048576
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
readonly CANONICAL_MAP_INPUT="${REPOSITORY_ROOT}/zion-corpus/data/books.json"

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
require_regular_input 'checked-in canonical book map' "${CANONICAL_MAP_INPUT}" \
    "${MAX_CANONICAL_MAP_BYTES}"

BOOK_FILENAME="$(basename -- "${BOOK_INPUT}")"
readonly BOOK_FILENAME
[[ "${BOOK_FILENAME}" =~ ^[0-9]{2}-[A-Z0-9]{3}\.json$ ]] ||
    die 'book JSON filename is not a canonical NN-ID.json leaf'
BOOK_DIRECTORY="$(cd -- "$(dirname -- "${BOOK_INPUT}")" && pwd -P)"
readonly BOOK_DIRECTORY
[[ "$(basename -- "${BOOK_DIRECTORY}")" == 'books' ]] ||
    die 'book JSON must be inside a materialized corpus books directory'
CORPUS_DIRECTORY="$(cd -- "${BOOK_DIRECTORY}/.." && pwd -P)"
readonly CORPUS_DIRECTORY
readonly MANIFEST_INPUT="${CORPUS_DIRECTORY}/manifest.json"
require_regular_input 'companion corpus manifest' "${MANIFEST_INPUT}" \
    "${MAX_MANIFEST_BYTES}"

CURRENT_UID="$(id -u)"
readonly CURRENT_UID
PASSPHRASE_STAT="$(stat -Lc '%u %a %s %d %i' -- "${PASSPHRASE_FILE}")" ||
    die 'could not pin passphrase-file metadata'
readonly PASSPHRASE_STAT
IFS=' ' read -r PASSPHRASE_UID PASSPHRASE_MODE PASSPHRASE_BYTES \
    PASSPHRASE_DEVICE PASSPHRASE_INODE <<<"${PASSPHRASE_STAT}"
readonly PASSPHRASE_UID PASSPHRASE_MODE PASSPHRASE_BYTES
readonly PASSPHRASE_DEVICE PASSPHRASE_INODE
[[ "${PASSPHRASE_UID}" == "${CURRENT_UID}" ]] ||
    die 'passphrase file must be owned by the current user'
[[ "${PASSPHRASE_MODE}" =~ ^[0-7]{3,4}$ ]] ||
    die 'could not validate passphrase-file permissions'
[[ "${PASSPHRASE_BYTES}" =~ ^[0-9]+$ &&
    "${PASSPHRASE_DEVICE}" =~ ^[0-9]+$ &&
    "${PASSPHRASE_INODE}" =~ ^[0-9]+$ ]] ||
    die 'could not validate passphrase-file identity'
((PASSPHRASE_BYTES > 0 && PASSPHRASE_BYTES <= MAX_PASSPHRASE_FILE_BYTES)) ||
    die 'passphrase file changed outside its validated byte bound'
readonly PASSPHRASE_MODE_VALUE=$((8#${PASSPHRASE_MODE}))
(((PASSPHRASE_MODE_VALUE & 0077) == 0)) ||
    die 'passphrase file must not be accessible by group or other users'
(((PASSPHRASE_MODE_VALUE & 0400) != 0)) ||
    die 'passphrase file must be readable by its owner'

PASSPHRASE_FD=''
PASSPHRASE_PROC_PATH=''
PASSPHRASE_FD_OPEN=0

iconv -f UTF-8 -t UTF-8 -- "${ATTRIBUTION_INPUT}" >/dev/null ||
    die 'attribution file must be valid UTF-8'
require_phone_rgb8_png 'clean cover' "${COVER_INPUT}"

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

readonly COVER_COPY="${OUTPUT_DIR}/clean-cover.png"
readonly SEALED_PNG="${OUTPUT_DIR}/sealed.arc.png"
readonly REENCODED_PNG="${OUTPUT_DIR}/sealed-compression0.arc.png"
readonly METRICS_FILE="${OUTPUT_DIR}/metrics.txt"
readonly TEMP_DIR="${OUTPUT_DIR}/.metrics-work"
readonly BOOK_COPY="${TEMP_DIR}/book.json"
readonly MANIFEST_COPY="${TEMP_DIR}/manifest.json"
readonly CANONICAL_MAP_COPY="${TEMP_DIR}/canonical-books.json"
readonly ATTRIBUTION_COPY="${TEMP_DIR}/attribution.txt"
readonly OPENED_JSON="${TEMP_DIR}/opened.json"
readonly REENCODED_OPENED_JSON="${TEMP_DIR}/compression0-opened.json"
readonly CLEAN_RGB="${TEMP_DIR}/clean.rgb"
readonly SEALED_RGB="${TEMP_DIR}/sealed.rgb"
readonly REENCODED_RGB="${TEMP_DIR}/reencoded.rgb"
readonly VERIFY_LOG="${TEMP_DIR}/verify.log"
readonly SEAL_LOG="${TEMP_DIR}/seal.log"
readonly CAPACITY_LOG="${TEMP_DIR}/capacity.log"
readonly PSNR_LOG="${TEMP_DIR}/psnr.log"
readonly SSIM_LOG="${TEMP_DIR}/ssim.log"

OUTPUT_CREATED=0
open_passphrase_fd() {
    local fd_stat
    local child_stat

    ((PASSPHRASE_FD_OPEN == 0)) ||
        die 'a passphrase descriptor is already open'
    [[ -f "${PASSPHRASE_FILE}" && ! -L "${PASSPHRASE_FILE}" &&
        -r "${PASSPHRASE_FILE}" ]] ||
        die 'the validated passphrase path is no longer a readable regular file'

    PASSPHRASE_FD=''
    exec {PASSPHRASE_FD}<"${PASSPHRASE_FILE}" ||
        die 'could not reopen the validated passphrase file'
    PASSPHRASE_FD_OPEN=1
    [[ "${PASSPHRASE_FD}" =~ ^[0-9]+$ ]] ||
        die 'Bash returned an invalid passphrase file descriptor'
    PASSPHRASE_PROC_PATH="/proc/self/fd/${PASSPHRASE_FD}"
    [[ -r "${PASSPHRASE_PROC_PATH}" && -f "${PASSPHRASE_PROC_PATH}" ]] ||
        die 'the pinned passphrase descriptor is not a readable regular file'

    fd_stat="$(stat -Lc '%u %a %s %d %i' -- "${PASSPHRASE_PROC_PATH}")" ||
        die 'could not validate the pinned passphrase descriptor'
    [[ "${fd_stat}" == "${PASSPHRASE_STAT}" ]] ||
        die 'passphrase path changed since its initial validation'
    child_stat="$(
        bash -c '
            [[ "$1" =~ ^/proc/self/fd/[0-9]+$ && -r "$1" && -f "$1" ]] || exit 1
            stat -Lc "%u %a %s %d %i" -- "$1"
        ' bash "${PASSPHRASE_PROC_PATH}"
    )" || die 'the pinned passphrase descriptor is not inherited by child processes'
    [[ "${child_stat}" == "${PASSPHRASE_STAT}" ]] ||
        die 'the inherited passphrase descriptor changed identity'
}

close_passphrase_fd() {
    local descriptor_path="${PASSPHRASE_PROC_PATH}"

    if ((PASSPHRASE_FD_OPEN == 1)); then
        exec {PASSPHRASE_FD}<&- || return 1
        PASSPHRASE_FD_OPEN=0
    fi
    [[ -z "${descriptor_path}" || ! -e "${descriptor_path}" ]] || return 1
    PASSPHRASE_FD=''
    PASSPHRASE_PROC_PATH=''
}

cleanup() {
    local status=$?
    local path
    trap - EXIT HUP INT TERM
    close_passphrase_fd || true

    for path in \
        "${BOOK_COPY}" "${MANIFEST_COPY}" "${CANONICAL_MAP_COPY}" \
        "${ATTRIBUTION_COPY}" \
        "${OPENED_JSON}" "${REENCODED_OPENED_JSON}" \
        "${CLEAN_RGB}" "${SEALED_RGB}" "${REENCODED_RGB}" \
        "${VERIFY_LOG}" "${SEAL_LOG}" \
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
            "${COVER_COPY}" "${SEALED_PNG}" "${REENCODED_PNG}" \
            "${METRICS_FILE}"; do
            if [[ -e "${path}" || -L "${path}" ]]; then
                rm -f -- "${path}" || true
            fi
        done
        if [[ -d "${OUTPUT_DIR}" && ! -L "${OUTPUT_DIR}" ]]; then
            rmdir -- "${OUTPUT_DIR}" ||
                printf '%s\n' \
                    'warning: incomplete private output directory could not be removed' >&2
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
cp -- "${MANIFEST_INPUT}" "${MANIFEST_COPY}"
cp -- "${CANONICAL_MAP_INPUT}" "${CANONICAL_MAP_COPY}"
cp -- "${ATTRIBUTION_INPUT}" "${ATTRIBUTION_COPY}"
cp -- "${COVER_INPUT}" "${COVER_COPY}"
chmod 0600 -- \
    "${BOOK_COPY}" "${MANIFEST_COPY}" "${CANONICAL_MAP_COPY}" \
    "${ATTRIBUTION_COPY}" "${COVER_COPY}"

cmp -s -- "${BOOK_INPUT}" "${BOOK_COPY}" ||
    die 'book JSON changed while it was being snapshotted'
cmp -s -- "${MANIFEST_INPUT}" "${MANIFEST_COPY}" ||
    die 'companion manifest changed while it was being snapshotted'
cmp -s -- "${CANONICAL_MAP_INPUT}" "${CANONICAL_MAP_COPY}" ||
    die 'canonical book map changed while it was being snapshotted'
cmp -s -- "${ATTRIBUTION_INPUT}" "${ATTRIBUTION_COPY}" ||
    die 'attribution changed while it was being snapshotted'
cmp -s -- "${COVER_INPUT}" "${COVER_COPY}" ||
    die 'cover changed while it was being snapshotted'

if ! python3 - \
    "${BOOK_COPY}" "${MANIFEST_COPY}" "${CANONICAL_MAP_COPY}" \
    "${ATTRIBUTION_COPY}" "${BOOK_FILENAME}" >"${VERIFY_LOG}" 2>&1 <<'PY'
import hashlib
import json
import pathlib
import re
import sys
import unicodedata

book_path = pathlib.Path(sys.argv[1])
manifest_path = pathlib.Path(sys.argv[2])
canonical_path = pathlib.Path(sys.argv[3])
attribution_path = pathlib.Path(sys.argv[4])
book_filename = sys.argv[5]

PINNED_MANIFEST_SHA256 = "84f99c4af8f4e0c63b39deb91c9928118123935f9f83127b9475abab544777cc"
PINNED_CANONICAL_SHA256 = "ec4225c67413072f5ceac8d5ca7a17a11e4dac5918ed39464f3ba3d359148d28"
PINNED_ATTRIBUTION_SHA256 = "459e4e5585e1ee8692f07aa947fe08b200f5e40afc09e46961fda42c5d5d070d"
PINNED_ATTRIBUTION_BYTES = 384
MAX_MARKERS = 8_192
MAX_TEXT_BYTES = 512 * 1024

SOURCE = {
    "name": "Bíblia Livre",
    "version": "2018.2.0",
    "version_date": "2018-02",
    "release_url": "https://github.com/blivre/BibliaLivre/releases/tag/2018.2.0",
    "asset_url": "https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip",
    "tag_commit": "a386942daee9984c654ebc8cea95ec9d3661b183",
    "license": "CC BY 3.0 BR",
    "license_url": "https://creativecommons.org/licenses/by/3.0/br/",
    "authors": ["Diego Santos", "Mario Sérgio", "Marco Teles"],
}
NORMALIZATION = (
    "The structured marker projection omits the UTF-8 BOM and normalizes CRLF "
    "to LF. The raw_usfm field preserves the verified source UTF-8 bytes exactly, "
    "including BOM and CRLF, and is exportable byte-for-byte."
)
SOURCE_ZIP = {
    "file": "usfm-blivre-tr.zip",
    "bytes": 1_364_020,
    "sha256": "83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead",
}
PROVENANCE = {
    "source_zip": "provenance/usfm-blivre-tr.zip",
    "upstream_readme": "provenance/UPSTREAM_README.md",
    "upstream_readme_sha256": "2c9188d1031b419ec4c59288f3aa41368bbfd5a4b646edaf6ea74169e1a46114",
    "upstream_license": "provenance/UPSTREAM_LICENSE.md",
    "upstream_license_sha256": "6ff396843c629e408e3dbad8b16653acbfed6448fe4011bbde4e13baaae706b7",
    "raw_usfm_directory": "provenance/raw-usfm",
    "normalization_notice": NORMALIZATION,
}
TOTALS = {
    "books": 66,
    "chapters": 1_189,
    "verse_records": 31_102,
    "raw_usfm_bytes": 4_276_761,
    "raw_usfm_zstd6_checksum_bytes": 1_391_039,
    "payload_json_bytes": 10_043_674,
    "payload_json_zstd6_bytes": 1_874_794,
}
CANONICAL_KEYS = {
    "ordinal", "id", "title", "source_file", "chapter_count", "verse_count",
    "raw_bytes", "raw_sha256",
}
MANIFEST_ENTRY_KEYS = {
    "ordinal", "id", "title", "source_file", "payload_file", "raw_usfm_file",
    "chapter_count", "verse_records", "marker_records", "raw_usfm_bytes",
    "raw_usfm_sha256", "raw_usfm_zstd6_checksum_bytes", "payload_json_bytes",
    "payload_json_sha256", "payload_json_zstd6_bytes",
}
BOOK_KEYS = {
    "schema", "id", "ordinal", "title", "language", "headers", "chapters",
    "raw_usfm", "source", "normalization",
}
SOURCE_KEYS = set(SOURCE)
HEADER_MARKERS = ("id", "ide", "h", "toc1", "toc2", "toc3")
BODY_MARKERS = {
    "p", "v", "d", "add", "add*", "f", "f*", "fr", "fq", "ft", "rq", "rq*",
}


def fail(message):
    raise ValueError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def reject_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            fail("duplicate JSON key")
        result[key] = value
    return result


def load_json(raw):
    try:
        text = raw.decode("utf-8")
        return json.loads(text, object_pairs_hook=reject_duplicate_keys)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("invalid JSON") from error


def exact_keys(value, keys, label):
    if type(value) is not dict or set(value) != keys:
        fail(f"invalid {label} shape")


def integer(value, minimum=0, maximum=None):
    if type(value) is not int or value < minimum:
        return False
    return maximum is None or value <= maximum


def safe_leaf(value):
    return (
        type(value) is str
        and bool(value)
        and value not in {".", ".."}
        and "/" not in value
        and "\\" not in value
        and "\x00" not in value
    )


def safe_relative(value):
    return (
        type(value) is str
        and bool(value)
        and not value.startswith("/")
        and "\\" not in value
        and "\x00" not in value
        and all(part not in {"", ".", ".."} for part in value.split("/"))
    )


def valid_text(value, maximum=MAX_TEXT_BYTES):
    return (
        type(value) is str
        and len(value.encode("utf-8")) <= maximum
        and not any(
            unicodedata.category(char) == "Cc" and char != "\n" for char in value
        )
    )


def validate_record(record, allowed_markers, header=False):
    if type(record) is not dict or not {"marker"} <= set(record) <= {"marker", "number", "text"}:
        fail("invalid marker record shape")
    marker = record["marker"]
    if type(marker) is not str or marker not in allowed_markers:
        fail("unrecognized marker")
    if "text" in record and not valid_text(record["text"]):
        fail("invalid marker text")
    if header:
        if "number" in record:
            fail("header number is forbidden")
    elif marker == "v":
        if not integer(record.get("number"), 1, 65_535):
            fail("invalid verse number")
    elif "number" in record:
        fail("number is valid only on a verse")
    return marker


manifest_raw = manifest_path.read_bytes()
canonical_raw = canonical_path.read_bytes()
book_raw = book_path.read_bytes()
attribution_raw = attribution_path.read_bytes()
if sha256(manifest_raw) != PINNED_MANIFEST_SHA256:
    fail("manifest identity mismatch")
if sha256(canonical_raw) != PINNED_CANONICAL_SHA256:
    fail("canonical mapping identity mismatch")
if (
    len(attribution_raw) != PINNED_ATTRIBUTION_BYTES
    or sha256(attribution_raw) != PINNED_ATTRIBUTION_SHA256
):
    fail("attribution identity mismatch")

manifest = load_json(manifest_raw)
canonical = load_json(canonical_raw)
book = load_json(book_raw)
exact_keys(
    manifest,
    {"schema", "source", "source_zip", "provenance", "totals", "books"},
    "manifest",
)
if manifest["schema"] != 1 or manifest["source"] != SOURCE:
    fail("unpinned manifest source")
if manifest["source_zip"] != SOURCE_ZIP or manifest["provenance"] != PROVENANCE:
    fail("unpinned manifest provenance")
if manifest["totals"] != TOTALS:
    fail("unpinned manifest totals")
if type(canonical) is not list or len(canonical) != 66:
    fail("canonical mapping must contain 66 books")
if type(manifest["books"]) is not list or len(manifest["books"]) != 66:
    fail("manifest must contain 66 books")

seen_ids = set()
seen_sources = set()
seen_paths = set()
selected = []
for index, (mapping, entry) in enumerate(
    zip(canonical, manifest["books"], strict=True), 1
):
    exact_keys(mapping, CANONICAL_KEYS, "canonical entry")
    exact_keys(entry, MANIFEST_ENTRY_KEYS, "manifest entry")
    if not integer(mapping["ordinal"], 1, 66) or mapping["ordinal"] != index:
        fail("non-contiguous canonical ordinals")
    if not re.fullmatch(r"[A-Z0-9]{3}", mapping["id"] or ""):
        fail("invalid canonical identifier")
    if not valid_text(mapping["title"], 255) or not mapping["title"]:
        fail("invalid canonical title")
    if not safe_leaf(mapping["source_file"]):
        fail("unsafe canonical source filename")
    for field in ("chapter_count", "verse_count", "raw_bytes"):
        if not integer(mapping[field], 1):
            fail("invalid canonical count")
    if not re.fullmatch(r"[0-9a-f]{64}", mapping["raw_sha256"] or ""):
        fail("invalid canonical raw hash")
    if mapping["id"] in seen_ids or mapping["source_file"] in seen_sources:
        fail("ambiguous canonical identity")
    seen_ids.add(mapping["id"])
    seen_sources.add(mapping["source_file"])

    expected_payload = f"books/{index:02d}-{mapping['id']}.json"
    expected_raw = f"provenance/raw-usfm/{mapping['source_file']}"
    if not safe_leaf(entry["source_file"]):
        fail("unsafe manifest source filename")
    if not safe_relative(entry["payload_file"]) or not safe_relative(entry["raw_usfm_file"]):
        fail("unsafe manifest path")
    if entry["payload_file"] in seen_paths or entry["raw_usfm_file"] in seen_paths:
        fail("ambiguous manifest path")
    seen_paths.update((entry["payload_file"], entry["raw_usfm_file"]))
    for left, right in (
        (entry["ordinal"], mapping["ordinal"]),
        (entry["id"], mapping["id"]),
        (entry["title"], mapping["title"]),
        (entry["source_file"], mapping["source_file"]),
        (entry["payload_file"], expected_payload),
        (entry["raw_usfm_file"], expected_raw),
        (entry["chapter_count"], mapping["chapter_count"]),
        (entry["verse_records"], mapping["verse_count"]),
        (entry["raw_usfm_bytes"], mapping["raw_bytes"]),
        (entry["raw_usfm_sha256"], mapping["raw_sha256"]),
    ):
        if left != right:
            fail("manifest/canonical mapping mismatch")
    for field in (
        "marker_records",
        "raw_usfm_zstd6_checksum_bytes",
        "payload_json_bytes",
        "payload_json_zstd6_bytes",
    ):
        if not integer(entry[field], 1):
            fail("invalid manifest metric")
    if not re.fullmatch(r"[0-9a-f]{64}", entry["payload_json_sha256"] or ""):
        fail("invalid payload hash")
    if entry["payload_file"] == f"books/{book_filename}":
        selected.append(entry)

if len(selected) != 1:
    fail("book has no unique canonical manifest entry")
entry = selected[0]
if (
    len(book_raw) != entry["payload_json_bytes"]
    or sha256(book_raw) != entry["payload_json_sha256"]
):
    fail("book payload identity mismatch")

exact_keys(book, BOOK_KEYS, "BookDocument")
if (
    book["schema"] != 1
    or book["id"] != entry["id"]
    or book["ordinal"] != entry["ordinal"]
    or book["title"] != entry["title"]
    or book["language"] != "pt-BR"
    or book["source"] != SOURCE
    or book["normalization"] != NORMALIZATION
):
    fail("BookDocument identity mismatch")
exact_keys(book["source"], SOURCE_KEYS, "BookDocument source")
if not valid_text(book["title"], 255) or not book["title"]:
    fail("invalid BookDocument title")
if type(book["headers"]) is not list or type(book["chapters"]) is not list:
    fail("invalid BookDocument collections")

header_markers = [
    validate_record(record, set(HEADER_MARKERS) | {"mt", "mt1"}, header=True)
    for record in book["headers"]
]
if (
    tuple(header_markers[:6]) != HEADER_MARKERS
    or len(header_markers) != 7
    or header_markers[6] not in {"mt", "mt1"}
):
    fail("invalid canonical header sequence")
if len(book["chapters"]) != entry["chapter_count"]:
    fail("chapter count mismatch")

# The importer metric counts each source `c` marker as well as the header/body
# markers; chapter boundaries are represented structurally in BookDocument.
marker_count = len(book["headers"]) + len(book["chapters"])
verse_count = 0
for chapter_index, chapter in enumerate(book["chapters"], 1):
    exact_keys(chapter, {"number", "markers"}, "chapter")
    if (
        chapter["number"] != chapter_index
        or type(chapter["markers"]) is not list
        or not chapter["markers"]
    ):
        fail("non-contiguous or empty chapter")
    addition = False
    footnote = False
    reference_quote = False
    last_verse = 0
    for record in chapter["markers"]:
        marker = validate_record(record, BODY_MARKERS)
        marker_count += 1
        if marker_count > MAX_MARKERS:
            fail("marker budget exceeded")
        if marker == "v":
            number = record["number"]
            if number <= last_verse:
                fail("non-increasing verse numbers")
            last_verse = number
            verse_count += 1
        elif marker == "add":
            if addition:
                fail("nested addition")
            addition = True
        elif marker == "add*":
            if not addition:
                fail("unmatched addition close")
            addition = False
        elif marker == "f":
            if footnote:
                fail("nested footnote")
            footnote = True
        elif marker == "f*":
            if not footnote:
                fail("unmatched footnote close")
            footnote = False
        elif marker in {"fr", "fq", "ft"} and not footnote:
            fail("footnote child outside footnote")
        elif marker == "rq":
            if reference_quote:
                fail("nested reference quote")
            reference_quote = True
        elif marker == "rq*":
            if not reference_quote:
                fail("unmatched reference quote close")
            reference_quote = False
    if addition or footnote or reference_quote or last_verse == 0:
        fail("unterminated inline marker or verse-free chapter")

if marker_count != entry["marker_records"] or verse_count != entry["verse_records"]:
    fail("renderable document metrics mismatch")
if type(book["raw_usfm"]) is not str or not book["raw_usfm"].startswith("\ufeff"):
    fail("raw USFM is not preserved UTF-8 with BOM")
raw_usfm = book["raw_usfm"].encode("utf-8")
if (
    len(raw_usfm) != entry["raw_usfm_bytes"]
    or sha256(raw_usfm) != entry["raw_usfm_sha256"]
):
    fail("raw USFM identity mismatch")
if b"\r\n" not in raw_usfm:
    fail("raw USFM line endings were not preserved")
PY
then
    die 'canonical BLIVRE package verification failed'
fi

IDENTITY="$(jq -er '[.ordinal, .id, .title] | @tsv' -- "${BOOK_COPY}")" ||
    die 'could not derive canonical book identity from the verified snapshot'
IFS=$'\t' read -r ORDINAL BOOK_ID BOOK_TITLE <<<"${IDENTITY}"
readonly ORDINAL BOOK_ID BOOK_TITLE
[[ "${ORDINAL}" =~ ^[0-9]+$ && "${BOOK_ID}" =~ ^[A-Z0-9]{3}$ &&
    -n "${BOOK_TITLE}" ]] || die 'verified canonical book identity is unusable'
printf -v ORDINAL_PADDED '%02d' "${ORDINAL}"
readonly ORDINAL_PADDED
readonly ITEM_NAME="${ORDINAL_PADDED} — ${BOOK_TITLE}"

require_phone_rgb8_png 'snapshotted clean cover' "${COVER_COPY}"

if ! "${ZION_EXECUTABLE}" arc capacity \
    --width "${PHONE_WIDTH}" \
    --height "${PHONE_HEIGHT}" \
    --profile safe >"${CAPACITY_LOG}" 2>&1; then
    die 'ARC capacity calculation failed'
fi

seal_start="$(date +%s%N)"
open_passphrase_fd
seal_status=0
if "${ZION_EXECUTABLE}" arc seal \
    "${BOOK_COPY}" \
    "${COVER_COPY}" \
    --output "${SEALED_PNG}" \
    --name "${ITEM_NAME}" \
    --media-type "${BOOK_MEDIA_TYPE}" \
    --attribution-file "${ATTRIBUTION_COPY}" \
    --profile safe \
    --passphrase-file "${PASSPHRASE_PROC_PATH}" >"${SEAL_LOG}" 2>&1; then
    :
else
    seal_status=$?
fi
close_passphrase_fd || die 'could not close the passphrase descriptor after sealing'
((seal_status == 0)) ||
    die 'ARC sealing failed; private CLI diagnostics were suppressed'
seal_finish="$(date +%s%N)"

CODEWORDS="$(
    sed -n 's/.*codewords=\([0-9][0-9]*\).*/\1/p' "${SEAL_LOG}"
)"
readonly CODEWORDS
CIPHERTEXT_BYTES="$(
    sed -n 's/.*ciphertext_bytes=\([0-9][0-9]*\).*/\1/p' "${SEAL_LOG}"
)"
readonly CIPHERTEXT_BYTES
KEY_DERIVATION_WALL_MS="$(
    sed -n 's/.*key_derivation_wall_ms=\([0-9][0-9]*\([.][0-9][0-9]*\)\?\).*/\1/p' \
        "${SEAL_LOG}"
)"
readonly KEY_DERIVATION_WALL_MS
[[ "${CODEWORDS}" =~ ^[0-9]+$ && "${CIPHERTEXT_BYTES}" =~ ^[0-9]+$ &&
    "${KEY_DERIVATION_WALL_MS}" =~ ^[0-9]+([.][0-9]+)?$ ]] ||
    die 'could not parse strict ARC seal metrics'
rm -f -- "${SEAL_LOG}"

open_start="$(date +%s%N)"
open_passphrase_fd
open_status=0
if "${ZION_EXECUTABLE}" arc open \
    "${SEALED_PNG}" \
    --output "${OPENED_JSON}" \
    --passphrase-file "${PASSPHRASE_PROC_PATH}" >/dev/null 2>&1; then
    :
else
    open_status=$?
fi
close_passphrase_fd || die 'could not close the passphrase descriptor after opening'
((open_status == 0)) ||
    die 'opening the first ARC PNG failed; private CLI diagnostics were suppressed'
open_finish="$(date +%s%N)"
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
    die 'compression-level-0 PNG is outside the 5,000,000..9,000,000 byte demo band'
require_phone_rgb8_png 'compression-level-0 ARC PNG' "${REENCODED_PNG}"

reopen_start="$(date +%s%N)"
open_passphrase_fd
reopen_status=0
if "${ZION_EXECUTABLE}" arc open \
    "${REENCODED_PNG}" \
    --output "${REENCODED_OPENED_JSON}" \
    --passphrase-file "${PASSPHRASE_PROC_PATH}" >/dev/null 2>&1; then
    :
else
    reopen_status=$?
fi
close_passphrase_fd || die 'could not close the passphrase descriptor after reopening'
((reopen_status == 0)) ||
    die 'opening the losslessly re-encoded ARC PNG failed; private CLI diagnostics were suppressed'
reopen_finish="$(date +%s%N)"
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
[[ "${CHANGED_CHANNELS}" =~ ^[0-9]+$ &&
    "${TOTAL_CHANNELS}" == "${PHONE_RGB_BYTES}" &&
    "${CHANGED_PERCENT}" =~ ^[0-9]+([.][0-9]+)?$ &&
    "${MEAN_ABSOLUTE_DELTA}" =~ ^[0-9]+([.][0-9]+)?$ &&
    "${MAX_DELTA}" == '1' ]] ||
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
[[ "${PSNR_DB}" =~ ^([0-9]+([.][0-9]+)?|inf)$ ]] ||
    die 'ffmpeg PSNR output was not a strict numeric value'

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
[[ "${SSIM}" =~ ^[0-9]+([.][0-9]+)?$ ]] ||
    die 'ffmpeg SSIM output was not a strict numeric value'

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
[[ "${MAX_CODEWORDS}" =~ ^[0-9]+$ && "${MAX_CIPHERTEXT_BYTES}" =~ ^[0-9]+$ ]] ||
    die 'could not parse strict ARC capacity metrics'

readonly INTERLEAVED_BYTES=$((CODEWORDS * 255))
readonly RS_PARITY_BYTES=$((INTERLEAVED_BYTES - CIPHERTEXT_BYTES))
readonly REQUIRED_PAYLOAD_BITS=$((INTERLEAVED_BYTES * 8))
COVER_PNG_BYTES="$(stat -Lc '%s' -- "${COVER_COPY}")"
readonly COVER_PNG_BYTES
SEALED_PNG_BYTES="$(stat -Lc '%s' -- "${SEALED_PNG}")"
readonly SEALED_PNG_BYTES
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
    printf 'canonical_blivre_package_validation=passed\n'
    printf 'canonical_attribution_validation=passed\n'
    printf 'dimensions=%sx%s\n' "${PHONE_WIDTH}" "${PHONE_HEIGHT}"
    printf 'pixel_format=rgb24\n'
    printf 'clean_bootstrap_magic_hex=%s\n' "${CLEAN_BOOTSTRAP_MAGIC}"
    printf 'clean_cover_status=no_ZARC_magic_before_seal\n'
    printf 'cover_png_bytes=%s\n' "${COVER_PNG_BYTES}"
    printf 'sealed_png_bytes=%s\n' "${SEALED_PNG_BYTES}"
    printf 'compression0_png_bytes=%s\n' "${REENCODED_PNG_BYTES}"
    printf 'compression0_size_band_bytes=5000000..9000000\n'
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
    printf 'key_derivation_wall_ms=%s\n' "${KEY_DERIVATION_WALL_MS}"
    printf 'seal_wall_ms=%s\n' "${SEAL_WALL_MS}"
    printf 'open_wall_ms=%s\n' "${OPEN_WALL_MS}"
    printf 'compression0_open_wall_ms=%s\n' "${REOPEN_WALL_MS}"
    printf 'cover_png_sha256=%s\n' "${COVER_PNG_SHA256}"
    printf 'sealed_png_sha256=%s\n' "${SEALED_PNG_SHA256}"
    printf 'compression0_png_sha256=%s\n' "${REENCODED_PNG_SHA256}"
    printf 'clean_rgb_sha256=%s\n' "${CLEAN_RGB_SHA256}"
    printf 'sealed_rgb_sha256=%s\n' "${SEALED_RGB_SHA256}"
    printf 'compression0_rgb_sha256=%s\n' "${REENCODED_RGB_SHA256}"
    printf 'first_open_cmp=exact\n'
    printf 'compression0_open_cmp=exact\n'
    printf 'sealed_to_compression0_rgb_identity=exact\n'
    printf 'demo_complete=yes\n'
} | tee "${METRICS_FILE}"

chmod 0600 -- \
    "${COVER_COPY}" "${SEALED_PNG}" "${REENCODED_PNG}" "${METRICS_FILE}"

for path in \
    "${BOOK_COPY}" "${MANIFEST_COPY}" "${CANONICAL_MAP_COPY}" \
    "${ATTRIBUTION_COPY}" \
    "${OPENED_JSON}" "${REENCODED_OPENED_JSON}" \
    "${CLEAN_RGB}" "${SEALED_RGB}" "${REENCODED_RGB}" \
    "${VERIFY_LOG}" "${SEAL_LOG}" \
    "${CAPACITY_LOG}" "${PSNR_LOG}" "${SSIM_LOG}"; do
    rm -f -- "${path}"
done
rmdir -- "${TEMP_DIR}"
((PASSPHRASE_FD_OPEN == 0)) && [[ -z "${PASSPHRASE_PROC_PATH}" ]] ||
    die 'a passphrase descriptor remained open after the final ARC invocation'

OUTPUT_CREATED=0
trap - EXIT HUP INT TERM
