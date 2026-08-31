package org.zioncode.app

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

internal val ZionNight = Color(0xFF04121A)
internal val ZionDeepBlue = Color(0xFF071D29)
internal val ZionPanel = Color(0xE6102935)
internal val ZionPanelRaised = Color(0xF2163542)
internal val ZionCyan = Color(0xFF47E6D1)
internal val ZionCyanSoft = Color(0xFF9AF4E7)
internal val ZionGold = Color(0xFFFFC857)
internal val ZionInk = Color(0xFFF2FBF9)
internal val ZionMuted = Color(0xFFA8C3C6)
internal val ZionLine = Color(0xFF29505C)
internal val ZionSuccess = Color(0xFF70E6A0)
internal val ZionError = Color(0xFFFF8B8B)

private val ZionColorScheme = darkColorScheme(
    primary = ZionCyan,
    onPrimary = Color(0xFF00201C),
    primaryContainer = Color(0xFF123E44),
    onPrimaryContainer = ZionCyanSoft,
    secondary = ZionGold,
    onSecondary = Color(0xFF2B1A00),
    background = ZionNight,
    onBackground = ZionInk,
    surface = ZionDeepBlue,
    onSurface = ZionInk,
    surfaceVariant = ZionPanelRaised,
    onSurfaceVariant = ZionMuted,
    outline = ZionLine,
    error = ZionError,
    onError = Color(0xFF330606),
)

private val ZionTypography = Typography(
    displaySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Black,
        fontSize = 38.sp,
        lineHeight = 41.sp,
        letterSpacing = (-0.8).sp,
    ),
    headlineSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 25.sp,
        lineHeight = 30.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 21.sp,
        lineHeight = 27.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 17.sp,
        lineHeight = 23.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 17.sp,
        lineHeight = 27.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 15.sp,
        lineHeight = 22.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.2.sp,
    ),
)

@Composable
internal fun ZionArcTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = ZionColorScheme,
        typography = ZionTypography,
        content = content,
    )
}
