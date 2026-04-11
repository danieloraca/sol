package com.soltv.bridge

import android.content.Context

object RustBridge {
  @Volatile private var initialized = false

  init {
    try {
      System.loadLibrary("sol")
    } catch (error: UnsatisfiedLinkError) {
      // Native library isn't bundled yet; UI can still boot.
    }
  }

  external fun nativeInitialize(storageDir: String, defaultAddonsJson: String): String
  external fun nativePing(): String
  external fun nativeGetInstalledAddonsJson(): String
  external fun nativeInstallAddonUrl(manifestUrl: String): String
  external fun nativeSetRemoteAddonEnabled(manifestUrl: String, enabled: Boolean): String
  external fun nativeRemoveRemoteAddon(manifestUrl: String): String
  external fun nativeMoveRemoteAddon(manifestUrl: String, direction: String): String
  external fun nativeGetHomeFeedJson(): String
  external fun nativeGetCatalogJson(): String
  external fun nativeGetStreamsJson(itemId: String): String

  fun ensureInitialized(context: Context) {
    if (initialized) return
    synchronized(this) {
      if (initialized) return
      try {
        val defaultAddons = context.assets.open("android.addons.seed.json")
          .bufferedReader()
          .use { it.readText() }
        nativeInitialize(context.filesDir.absolutePath, defaultAddons)
      } catch (_: Throwable) {
        nativeInitialize(context.filesDir.absolutePath, "")
      }
      initialized = true
    }
  }

  fun pingOrFallback(): String {
    return try {
      nativePing()
    } catch (_: UnsatisfiedLinkError) {
      "sol native core not loaded"
    }
  }

  fun homeFeedJsonOrFallback(): String {
    return try {
      nativeGetHomeFeedJson()
    } catch (_: UnsatisfiedLinkError) {
      """{"hero":{"title":"Native library not loaded"},"trending":[]}"""
    } catch (_: Throwable) {
      """{"hero":{"title":"Could not load home feed"},"trending":[]}"""
    }
  }

  fun catalogJsonOrFallback(): String {
    return try {
      nativeGetCatalogJson()
    } catch (_: UnsatisfiedLinkError) {
      "[]"
    } catch (_: Throwable) {
      "[]"
    }
  }

  fun streamsJsonOrFallback(itemId: String): String {
    val nativeJson = try {
      nativeGetStreamsJson(itemId)
    } catch (_: Throwable) {
      ""
    }.trim()

    if (nativeJson.isNotEmpty()) {
      return nativeJson
    }

    return """{
      "provider":"Addons",
      "status":"unavailable",
      "message":"Native stream lookup unavailable.",
      "streams":[],
      "candidates":[]
    }"""
  }
}
