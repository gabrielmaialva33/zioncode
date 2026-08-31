package org.zioncode.app

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ZionArcAppTest {
    @Test
    fun freshLaunchStartsThePermissionAndDocumentFlow() {
        assertTrue(shouldStartInitialAccessFlow(alreadyRequested = false, hasSelection = false))
    }

    @Test
    fun accessFlowDoesNotLoopAfterCancelOrSelection() {
        assertFalse(shouldStartInitialAccessFlow(alreadyRequested = true, hasSelection = false))
        assertFalse(shouldStartInitialAccessFlow(alreadyRequested = true, hasSelection = true))
    }

    @Test
    fun imagePermissionsFollowAndroidStorageGenerations() {
        assertArrayEquals(
            arrayOf(READ_EXTERNAL_STORAGE_PERMISSION),
            requiredImagePermissions(32),
        )
        assertArrayEquals(
            arrayOf(READ_MEDIA_IMAGES_PERMISSION),
            requiredImagePermissions(33),
        )
        assertArrayEquals(
            arrayOf(
                READ_MEDIA_IMAGES_PERMISSION,
                READ_MEDIA_VISUAL_USER_SELECTED_PERMISSION,
            ),
            requiredImagePermissions(34),
        )
    }
}
