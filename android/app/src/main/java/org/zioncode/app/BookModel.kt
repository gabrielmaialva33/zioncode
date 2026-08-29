package org.zioncode.app

import org.json.JSONArray
import org.json.JSONObject

internal const val RECOVERY_FAILURE_CODE: String = "ZION_RECOVERY_FAILED"

internal data class BookUiModel(
    val id: String,
    val ordinal: Int,
    val title: String,
    val sourceName: String,
    val sourceVersion: String,
    val rows: List<ReaderRow>,
    val rawUsfm: String,
)

internal sealed interface ReaderRow {
    data class ChapterHeading(val number: Int) : ReaderRow
    data class Verse(val number: Int, val text: String) : ReaderRow
    data class Marker(val name: String, val text: String) : ReaderRow
    data object Paragraph : ReaderRow
}

internal fun parseRecoveredBook(jsonBytes: ByteArray): BookUiModel = try {
    parseValidatedBook(jsonBytes)
} catch (_: Exception) {
    throw IllegalStateException(RECOVERY_FAILURE_CODE)
}

private fun parseValidatedBook(jsonBytes: ByteArray): BookUiModel {
    val root = JSONObject(jsonBytes.toString(Charsets.UTF_8))
    val source = root.getJSONObject("source")
    val rows = buildList {
        val chapters = root.getJSONArray("chapters")
        for (chapterIndex in 0 until chapters.length()) {
            val chapter = chapters.getJSONObject(chapterIndex)
            add(ReaderRow.ChapterHeading(chapter.getInt("number")))
            addMarkers(chapter.getJSONArray("markers"))
        }
    }
    return BookUiModel(
        id = root.getString("id"),
        ordinal = root.getInt("ordinal"),
        title = root.getString("title"),
        sourceName = source.getString("name"),
        sourceVersion = source.getString("version"),
        rows = rows,
        rawUsfm = root.getString("raw_usfm"),
    )
}

private fun MutableList<ReaderRow>.addMarkers(markers: JSONArray) {
    for (index in 0 until markers.length()) {
        val marker = markers.getJSONObject(index)
        val name = marker.getString("marker")
        val text = marker.optString("text", "")
        when (name) {
            "p" -> add(ReaderRow.Paragraph)
            "v" -> add(ReaderRow.Verse(marker.getInt("number"), text))
            else -> add(ReaderRow.Marker(name, text))
        }
    }
}
