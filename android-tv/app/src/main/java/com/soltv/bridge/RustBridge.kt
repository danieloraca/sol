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
}
