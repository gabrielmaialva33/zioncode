package org.zioncode.app

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.IOException
import java.io.InputStream

class BoundedInputTest {
    @Test
    fun emptyInputAtZeroLimitIsAccepted() {
        assertArrayEquals(
            byteArrayOf(),
            readBounded(ByteArrayInputStream(byteArrayOf()), 0),
        )
    }

    @Test
    fun exactLimitIsAccepted() {
        val input = byteArrayOf(1, 2, 3, 4)
        assertArrayEquals(input, readBounded(ByteArrayInputStream(input), input.size))
    }

    @Test
    fun firstBytePastLimitIsRejected() {
        val input = byteArrayOf(1, 2, 3, 4, 5)
        assertThrows(IOException::class.java) {
            readBounded(ByteArrayInputStream(input), 4)
        }
    }

    @Test
    fun shortReadsPreserveEveryByte() {
        val expected = byteArrayOf(9, 8, 7, 6, 5)
        val oneByteAtATime = object : InputStream() {
            var position = 0

            override fun read(): Int =
                if (position == expected.size) -1 else expected[position++].toInt() and 0xff

            override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
                if (position == expected.size) return -1
                buffer[offset] = expected[position++]
                return 1
            }
        }

        assertArrayEquals(expected, readBounded(oneByteAtATime, expected.size))
    }

    @Test
    fun documentSizesUseCompactStableUnits() {
        assertEquals("Tamanho não informado", formatDocumentSize(null))
        assertEquals("900 bytes", formatDocumentSize(900))
        assertEquals("1.5 KB", formatDocumentSize(1_536))
        assertEquals("5.0 MB", formatDocumentSize(5_242_880))
    }
}
