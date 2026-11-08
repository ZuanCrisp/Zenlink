package dev.zenshiro.zenlink

import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import dev.zenshiro.zenlink.ui.HomeScreen
import dev.zenshiro.zenlink.ui.ReceiveScreen
import dev.zenshiro.zenlink.ui.SendScreen
import dev.zenshiro.zenlink.ui.ZenLinkTheme
import dev.zenshiro.zenlink.ui.Zl
import java.util.Locale

/** Screens of the app. A TV opens straight on Receive. */
enum class Screen { Home, Send, Receive }

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val tv = isTv(this)
        setContent {
            ZenLinkTheme {
                Surface(Modifier.fillMaxSize()) {
                    Box(Modifier.fillMaxSize().background(Zl.Bg)) {
                        var screen by rememberSaveable { mutableStateOf(if (tv) Screen.Receive else Screen.Home) }
                        BackHandler(enabled = screen != Screen.Home && !tv) { screen = Screen.Home }
                        when (screen) {
                            Screen.Home -> HomeScreen(tv = tv, open = { screen = it })
                            Screen.Send -> SendScreen(back = { screen = Screen.Home })
                            Screen.Receive -> ReceiveScreen(tv = tv, back = { screen = Screen.Home })
                        }
                    }
                }
            }
        }
    }

    companion object {
        fun isTv(context: Context): Boolean {
            val ui = context.getSystemService(Context.UI_MODE_SERVICE) as UiModeManager
            return ui.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION ||
                context.packageManager.hasSystemFeature("android.software.leanback")
        }

        /** Name other devices see: the user-set device name when there is one, else the model. */
        fun deviceName(context: Context): String {
            val set = runCatching {
                android.provider.Settings.Global.getString(context.contentResolver, "device_name")
            }.getOrNull()
            return set?.takeIf { it.isNotBlank() } ?: Build.MODEL
        }
    }
}

/** Indonesian when the device language is Indonesian, otherwise English (as on the desktop). */
fun tr(id: String, en: String): String =
    if (Locale.getDefault().language.let { it == "id" || it == "in" }) id else en
