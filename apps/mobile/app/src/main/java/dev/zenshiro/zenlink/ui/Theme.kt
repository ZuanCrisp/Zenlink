package dev.zenshiro.zenlink.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** ZenLink colours, the same as the desktop app: near-black with pink accents. */
object Zl {
    val Bg = Color(0xFF0E0E12)
    val Card = Color(0xFF17171D)
    val Line = Color(0xFF2A2A33)
    val Pink = Color(0xFFFF4FA3)
    val Text = Color(0xFFF2F2F5)
    val Dim = Color(0xFF9A9AA6)
    val Error = Color(0xFFFF6B6B)
}

@Composable
fun ZenLinkTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Zl.Pink,
            onPrimary = Color.White,
            background = Zl.Bg,
            onBackground = Zl.Text,
            surface = Zl.Card,
            onSurface = Zl.Text,
            surfaceVariant = Zl.Card,
            onSurfaceVariant = Zl.Dim,
            outline = Zl.Line,
            error = Zl.Error,
        ),
        content = content,
    )
}
