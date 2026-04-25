package com.soltv

import android.media.AudioManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.soltv.bridge.RustBridge
import com.soltv.ui.SolTvApp

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    volumeControlStream = AudioManager.STREAM_MUSIC
    RustBridge.ensureInitialized(this)

    setContent {
      SolTvApp(
        nativeStatus = RustBridge.pingOrFallback(),
        homeFeedJson = RustBridge.homeFeedJsonOrFallback(),
        catalogJson = RustBridge.catalogJsonOrFallback(),
      )
    }
  }
}
