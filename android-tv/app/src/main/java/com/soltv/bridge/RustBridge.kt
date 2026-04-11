package com.soltv.bridge

object RustBridge {
  init {
    try {
      System.loadLibrary("sol")
    } catch (error: UnsatisfiedLinkError) {
      // Native library isn't bundled yet; UI can still boot.
    }
  }

  external fun nativePing(): String
  external fun nativeGetHomeFeedJson(): String
  external fun nativeGetCatalogJson(): String
  external fun nativeGetStreamsJson(itemId: String): String

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
    val safeItemId = itemId.replace("\"", "")
    return """{
      "status":"ready",
      "message":"Demo stream for $safeItemId",
      "streams":[
        {
          "name":"Demo Stream (HLS)",
          "url":"https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
          "playback_kind":"embedded"
        },
        {
          "name":"Demo Stream (MP4 Fallback)",
          "url":"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4",
          "playback_kind":"embedded"
        }
      ]
    }"""
  }
}
