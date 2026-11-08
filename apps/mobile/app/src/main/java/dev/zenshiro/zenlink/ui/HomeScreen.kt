package dev.zenshiro.zenlink.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import dev.zenshiro.zenlink.Screen
import dev.zenshiro.zenlink.tr

@Composable
fun HomeScreen(tv: Boolean, open: (Screen) -> Unit) {
    val first = remember { FocusRequester() }
    // D-pad: start with a focused button so the remote works at once.
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Page(
        title = "ZenLink",
        subtitle = tr(
            "Kirim layar HP ke perangkat lain di Wi-Fi yang sama, atau jadikan perangkat ini layar penerima.",
            "Send this screen to another device on the same Wi-Fi, or make this device the receiving screen.",
        ),
    ) {
        ZlCard {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (!tv) {
                    Button(
                        onClick = { open(Screen.Send) },
                        modifier = Modifier.fillMaxWidth().focusRequester(first),
                        colors = ButtonDefaults.buttonColors(containerColor = Zl.Pink),
                    ) { Text(tr("Kirim layar ini", "Send this screen")) }
                }
                OutlinedButton(
                    onClick = { open(Screen.Receive) },
                    modifier = if (tv) Modifier.fillMaxWidth().focusRequester(first) else Modifier.fillMaxWidth(),
                ) { Text(tr("Terima layar (jadi penerima)", "Receive a screen")) }
            }
        }
    }
}
