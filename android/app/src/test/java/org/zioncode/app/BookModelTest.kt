package org.zioncode.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class BookModelTest {
    @Test
    fun sharedRustBookDocumentProjectsEveryReaderRowAndRawExportByte() {
        val fixture = requireNotNull(
            BookModelTest::class.java.getResourceAsStream("/book_document_v1.json"),
        ).use { it.readBytes() }

        val book = parseRecoveredBook(fixture)

        assertEquals("TST", book.id)
        assertEquals(7, book.ordinal)
        assertEquals("Guia de Horta", book.title)
        assertEquals("Coleção Sintética", book.sourceName)
        assertEquals("2026.08-contract", book.sourceVersion)
        assertEquals(
            listOf(
                ReaderRow.ChapterHeading(1),
                ReaderRow.Paragraph,
                ReaderRow.Marker("d", "Preparação"),
                ReaderRow.Verse(1, "Prepare o vaso."),
                ReaderRow.Marker("add", "com cuidado"),
                ReaderRow.Marker("add*", ""),
                ReaderRow.Verse(2, "Registre o resultado."),
                ReaderRow.ChapterHeading(2),
                ReaderRow.Marker("d", "Revisão"),
                ReaderRow.Paragraph,
                ReaderRow.Verse(1, "Observe o crescimento."),
                ReaderRow.Marker("rq", "consulte a ficha"),
                ReaderRow.Marker("rq*", ""),
            ),
            book.rows,
        )
        assertEquals(EXPECTED_RAW_USFM, book.rawUsfm)
        assertEquals(
            EXPECTED_RAW_USFM.toByteArray(Charsets.UTF_8).toList(),
            book.rawUsfm.toByteArray(Charsets.UTF_8).toList(),
        )
    }

    @Test
    fun kotlinJsonFailureUsesTheGenericRecoveryContract() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseRecoveredBook("{not-json".toByteArray(Charsets.UTF_8))
        }

        assertEquals(RECOVERY_FAILURE_CODE, failure.message)
        assertNull(failure.cause)
    }

    private companion object {
        const val EXPECTED_RAW_USFM: String =
            "\uFEFF\\id TST\r\n" +
                "\\ide UTF-8\r\n" +
                "\\h Guia de Horta\r\n" +
                "\\toc1 Guia de Horta\r\n" +
                "\\toc2 Guia de Horta\r\n" +
                "\\toc3 TST\r\n" +
                "\\mt Guia de Horta\r\n" +
                "\\c 1\r\n" +
                "\\p\r\n" +
                "\\d Preparação\r\n" +
                "\\v 1 Prepare o vaso.\\add com cuidado\\add*\r\n" +
                "\\v 2 Registre o resultado.\r\n" +
                "\\c 2\r\n" +
                "\\d Revisão\r\n" +
                "\\p\r\n" +
                "\\v 1 Observe o crescimento.\\rq consulte a ficha\\rq*\r\n"
    }
}
