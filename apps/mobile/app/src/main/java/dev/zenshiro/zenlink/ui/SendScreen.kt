package dev.zenshiro.zenlink.ui

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import dev.zenshiro.zenlink.MainActivity
import dev.zenshiro.zenlink.net.Discovery
import dev.zenshiro.zenlink.net.Link
import dev.zenshiro.zenlink.net.Peer
import dev.zenshiro.zenlink.net.Wire
import dev.zenshiro.zenlink.net.WrongCodeException
import dev.zenshiro.zenlink.tr
import kotlin.concurrent.thread

/**
 * Sender: lists receivers on the network, asks for the code shown there and
 * opens the link. Screen capture (MediaProjection) is phase 8.
 */
@Composable
fun SendScreen(back: () -> Unit) {
    val context = LocalContext.current
    val name = remember { MainActivity.deviceName(context) }
    var peers by remember { mutableStateOf(emptyList<Peer>()) }
    var chosen by remember { mutableStateOf<Peer?>(null) }
    var code by remember { mutableStateOf("") }
    var status by remember { mutableStateOf<String?>(null) }
    var open by remember { mutableStateOf<Link.Open?>(null) }

    DisposableEffect(Unit) {
        val discovery = Discovery(context)
        discovery.browse { peers = it }
        onDispose {
            discovery.close()
            open?.let { o -> thread { runCatching { o.link.send(Wire.BYE, byteArrayOf(0)) }; o.close() } }
        }
    }

    Page(
        title = tr("Kirim layar ini", "Send this screen"),
        subtitle = tr(
            "Penerima harus membuka ZenLink > Terima layar, di Wi-Fi yang sama.",
            "The receiver must open ZenLink > Receive a screen, on the same Wi-Fi.",
        ),
        back = back,
    ) {
        val target = chosen
        val linked = open
        when {
            linked != null -> ZlCard {
                Text(tr("Terhubung ke ", "Connected to ") + linked.link.peerName, color = Zl.Text)
                Text(tr("Kirim video menyusul (fase 8).", "Video sending comes next (phase 8)."), color = Zl.Dim)
                OutlinedButton(onClick = {
                    thread { runCatching { linked.link.send(Wire.BYE, byteArrayOf(0)) }; linked.close() }
                    open = null
                    chosen = null
                }) { Text(tr("Putuskan", "Disconnect")) }
            }
            target != null -> ZlCard {
                Text(tr("Kode di ", "Code on ") + target.name, color = Zl.Text)
                OutlinedTextField(
                    value = code,
                    onValueChange = { v -> code = v.filter(Char::isDigit).take(6) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    modifier = Modifier.fillMaxWidth(),
                )
                status?.let { Text(it, color = Zl.Error) }
                Button(
                    enabled = code.length == 6,
                    colors = ButtonDefaults.buttonColors(containerColor = Zl.Pink),
                    onClick = {
                        status = tr("Menghubungkan…", "Connecting…")
                        val c = code
                        thread(name = "zenlink-connect", isDaemon = true) {
                            try {
                                open = Link.connect(target.host, target.port, name, c)
                                status = null
                            } catch (e: WrongCodeException) {
                                status = tr("Kode salah.", "Wrong code.")
                            } catch (e: Exception) {
                                status = tr("Gagal terhubung: ", "Could not connect: ") + (e.message ?: e.javaClass.simpleName)
                            }
                        }
                    },
                ) { Text(tr("Hubungkan", "Connect")) }
                OutlinedButton(onClick = { chosen = null; code = ""; status = null }) { Text(tr("Pilih lain", "Pick another")) }
            }
            else -> ZlCard {
                if (peers.isEmpty()) {
                    Text(tr("Mencari penerima…", "Looking for receivers…"), color = Zl.Dim)
                }
                peers.forEach { p ->
                    OutlinedButton(onClick = { chosen = p }, modifier = Modifier.fillMaxWidth()) {
                        Text((if (p.tv) "📺 " else "📱 ") + p.name)
                    }
                }
            }
        }
    }
}
