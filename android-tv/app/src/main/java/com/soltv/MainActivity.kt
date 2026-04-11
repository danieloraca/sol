package com.soltv

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.soltv.bridge.RustBridge
import com.soltv.ui.SolTvApp

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
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
