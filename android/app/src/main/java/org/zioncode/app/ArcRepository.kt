package org.zioncode.app

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.util.Locale

internal const val MAX_ARC_PNG_BYTES: Int = 134_217_728
internal const val MAX_ARC_PASSPHRASE_BYTES: Int = 1_024

internal data class SelectedDocumentInfo(
    val displayName: String,
    val sizeBytes: Long?,
)

internal class ArcRepository(private val resolver: ContentResolver) {
    suspend fun describe(uri: Uri): SelectedDocumentInfo = withContext(Dispatchers.IO) {
        runCatching {
            resolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE),
                null,
                null,
                null,
            )?.use { cursor ->
                if (!cursor.moveToFirst()) return@use null
                val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE)
                val displayName = if (nameIndex >= 0 && !cursor.isNull(nameIndex)) {
                    cursor.getString(nameIndex).take(120)
                } else {
                    null
                }
                val sizeBytes = if (sizeIndex >= 0 && !cursor.isNull(sizeIndex)) {
                    cursor.getLong(sizeIndex).takeIf { it >= 0 }
                } else {
                    null
                }
                SelectedDocumentInfo(
                    displayName = displayName.orEmpty().ifBlank { "Arte PNG selecionada" },
                    sizeBytes = sizeBytes,
                )
            }
        }.getOrNull() ?: SelectedDocumentInfo("Arte PNG selecionada", null)
    }

    suspend fun decode(uri: Uri, passphraseBytes: ByteArray): BookUiModel {
        var pngBytes: ByteArray? = null
        var jsonBytes: ByteArray? = null
        try {
            pngBytes = withContext(Dispatchers.IO) {
                resolver.openInputStream(uri)?.use { stream ->
                    readBounded(stream, MAX_ARC_PNG_BYTES)
                } ?: throw IOException("The selected document could not be opened")
            }
            jsonBytes = withContext(Dispatchers.Default) {
                NativeBridge.decodeBookPng(pngBytes, passphraseBytes)
            }
            return withContext(Dispatchers.Default) {
                parseRecoveredBook(jsonBytes)
            }
        } finally {
            pngBytes?.fill(0)
            jsonBytes?.fill(0)
        }
    }

    suspend fun exportRawUsfm(uri: Uri, rawUsfm: String) {
        val bytes = rawUsfm.toByteArray(Charsets.UTF_8)
        try {
            withContext(Dispatchers.IO) {
                resolver.openOutputStream(uri, "w")?.use { stream ->
                    stream.write(bytes)
                    stream.flush()
                } ?: throw IOException("The export document could not be opened")
            }
        } finally {
            bytes.fill(0)
        }
    }
}

internal fun formatDocumentSize(sizeBytes: Long?): String = when {
    sizeBytes == null -> "Tamanho não informado"
    sizeBytes >= 1_048_576 -> String.format(Locale.ROOT, "%.1f MB", sizeBytes / 1_048_576.0)
    sizeBytes >= 1_024 -> String.format(Locale.ROOT, "%.1f KB", sizeBytes / 1_024.0)
    else -> "$sizeBytes bytes"
}

internal fun isPassphraseInputWithinLimit(value: String): Boolean {
    val utf8 = value.toByteArray(Charsets.UTF_8)
    return try {
        utf8.size <= MAX_ARC_PASSPHRASE_BYTES
    } finally {
        utf8.fill(0)
    }
}

@Throws(IOException::class)
internal fun readBounded(input: InputStream, maximumBytes: Int): ByteArray {
    require(maximumBytes >= 0)
    val output = ByteArrayOutputStream(minOf(maximumBytes, 64 * 1024))
    val buffer = ByteArray(64 * 1024)
    var total = 0
    while (true) {
        val remainingProbe = maximumBytes - total + 1
        val read = input.read(buffer, 0, minOf(buffer.size, remainingProbe))
        if (read == -1) {
            return output.toByteArray()
        }
        total += read
        if (total > maximumBytes) {
            throw IOException("The selected document exceeds the ARC limit")
        }
        output.write(buffer, 0, read)
    }
}
