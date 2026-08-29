package org.zioncode.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PasswordInputTest {
    @Test
    fun emptyAndExactUtf8LimitAreAcceptedForEditing() {
        assertTrue(isPassphraseInputWithinLimit(""))
        assertTrue(isPassphraseInputWithinLimit("a".repeat(MAX_ARC_PASSPHRASE_BYTES)))
        assertTrue(isPassphraseInputWithinLimit("á".repeat(MAX_ARC_PASSPHRASE_BYTES / 2)))
    }

    @Test
    fun firstUtf8BytePastLimitIsRejected() {
        assertFalse(
            isPassphraseInputWithinLimit("a".repeat(MAX_ARC_PASSPHRASE_BYTES + 1)),
        )
        assertFalse(
            isPassphraseInputWithinLimit("á".repeat(MAX_ARC_PASSPHRASE_BYTES / 2) + "a"),
        )
    }
}
