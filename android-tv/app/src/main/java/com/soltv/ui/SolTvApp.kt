package com.soltv.ui

import android.net.Uri
import android.widget.MediaController
import android.widget.VideoView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import coil.compose.AsyncImage
import com.soltv.bridge.RustBridge
import org.json.JSONArray
import org.json.JSONObject

@Composable
fun SolTvApp(nativeStatus: String, homeFeedJson: String, catalogJson: String) {
  val feed = parseHomeFeedSnapshot(homeFeedJson)
  val catalog = parseCatalogSnapshot(catalogJson)
  var playback by remember { mutableStateOf<PlaybackSelection?>(null) }
  var playbackMessage by remember { mutableStateOf<String?>(null) }

  MaterialTheme {
    Surface(modifier = Modifier.fillMaxSize()) {
      if (playback != null) {
        VideoViewPlayerScreen(
          selection = playback!!,
          onBack = { playback = null },
          onPlaybackError = { error ->
            playback = null
            playbackMessage = "Playback failed: $error"
          },
        )
      } else {
        LazyColumn(
          modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 48.dp, vertical = 36.dp),
          verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
          item {
            Text(text = "Sol for Android TV", style = MaterialTheme.typography.headlineLarge)
          }
          item {
            Text(text = "Native bridge: $nativeStatus")
          }
          item {
            Text(
              text = "Hero: ${feed.heroTitle}",
              style = MaterialTheme.typography.titleLarge,
            )
          }
          item {
            Text(text = "Trending", style = MaterialTheme.typography.titleMedium)
          }
          if (feed.trendingTitles.isEmpty()) {
            item {
              Text(text = "No trending titles returned from Rust yet.")
            }
          } else {
            items(feed.trendingTitles) { title ->
              Text(text = "• $title")
            }
          }

          item {
            Text(text = "Catalog", style = MaterialTheme.typography.titleMedium)
          }

          item {
            if (catalog.isEmpty()) {
              Text(text = "No catalog items returned from Rust yet.")
            } else {
              LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                items(catalog, key = { item -> item.id }) { item ->
                  PosterCard(
                    item = item,
                    onClick = {
                      val stream = parseFirstStream(RustBridge.streamsJsonOrFallback(item.id))
                      if (stream == null) {
                        playbackMessage = "No playable stream found for ${item.title}."
                        return@PosterCard
                      }

                      playbackMessage = null
                      playback = PlaybackSelection(
                        title = item.title,
                        streamName = stream.name,
                        streamUrl = stream.url,
                      )
                    },
                  )
                }
              }
            }
          }

          if (!playbackMessage.isNullOrBlank()) {
            item {
              Text(text = playbackMessage ?: "", color = Color(0xFFFFB4AB))
            }
          }
        }
      }
    }
  }
}

@Composable
private fun VideoViewPlayerScreen(
  selection: PlaybackSelection,
  onBack: () -> Unit,
  onPlaybackError: (String) -> Unit,
) {
  val context = LocalContext.current

  BackHandler(onBack = onBack)

  Box(modifier = Modifier.fillMaxSize()) {
    AndroidView(
      modifier = Modifier.fillMaxSize(),
      factory = { viewContext ->
        VideoView(viewContext).apply {
          val controller = MediaController(viewContext)
          controller.setAnchorView(this)
          setMediaController(controller)

          setOnPreparedListener { mediaPlayer ->
            mediaPlayer.isLooping = false
            start()
          }

          setOnErrorListener { _, what, extra ->
            onPlaybackError("what=$what extra=$extra")
            true
          }

          setVideoURI(Uri.parse(selection.streamUrl))
          requestFocus()
          start()
        }
      },
      update = { videoView ->
        if (!videoView.isPlaying) {
          videoView.start()
        }
      },
    )

    Box(
      modifier = Modifier
        .align(Alignment.TopStart)
        .padding(16.dp)
        .background(Color(0xB3000000), RoundedCornerShape(10.dp))
        .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
      Text(text = "${selection.title} • ${selection.streamName}", color = Color.White)
    }
  }
}

@Composable
private fun PosterCard(item: CatalogItem, onClick: () -> Unit) {
  var isFocused by remember { mutableStateOf(false) }
  val scale by animateFloatAsState(targetValue = if (isFocused) 1.05f else 1.0f, label = "posterScale")

  Box(
    modifier = Modifier
      .width(180.dp)
      .height(270.dp)
      .scale(scale)
      .clip(RoundedCornerShape(14.dp))
      .background(Color(0xFF1C2B3A))
      .onFocusChanged { state ->
        isFocused = state.isFocused
      }
      .focusable()
      .clickable(
        interactionSource = remember { MutableInteractionSource() },
        indication = null,
      ) {
        onClick()
      },
  ) {
    if (item.posterUrl.isNullOrBlank()) {
      Box(
        modifier = Modifier
          .fillMaxSize()
          .background(Color(0xFF2A3F55)),
      )
    } else {
      AsyncImage(
        model = item.posterUrl,
        contentDescription = "${item.title} poster",
        contentScale = ContentScale.Crop,
        modifier = Modifier.fillMaxSize(),
      )
    }

    Box(
      modifier = Modifier
        .align(Alignment.BottomStart)
        .background(Color(0xCC031321))
        .padding(horizontal = 10.dp, vertical = 8.dp),
    ) {
      Text(
        text = item.title,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
        style = MaterialTheme.typography.bodyMedium,
        color = Color.White,
      )
    }
  }
}

private data class HomeFeedSnapshot(
  val heroTitle: String,
  val trendingTitles: List<String>,
)

private data class CatalogItem(
  val id: String,
  val title: String,
  val posterUrl: String?,
)

private data class PlaybackSelection(
  val title: String,
  val streamName: String,
  val streamUrl: String,
)

private data class StreamInfo(
  val name: String,
  val url: String,
)

private fun parseHomeFeedSnapshot(rawJson: String): HomeFeedSnapshot {
  return try {
    val root = JSONObject(rawJson)
    val heroTitle = root.optJSONObject("hero")?.optString("title").orEmpty()
      .ifBlank { "Unknown title" }
    val trendingArray = root.optJSONArray("trending")
    val trendingTitles = buildList {
      if (trendingArray != null) {
        for (index in 0 until trendingArray.length()) {
          val title = trendingArray.optJSONObject(index)?.optString("title").orEmpty().trim()
          if (title.isNotEmpty()) {
            add(title)
          }
        }
      }
    }

    HomeFeedSnapshot(heroTitle = heroTitle, trendingTitles = trendingTitles)
  } catch (_: Throwable) {
    HomeFeedSnapshot(heroTitle = "Home feed parse error", trendingTitles = emptyList())
  }
}

private fun parseCatalogSnapshot(rawJson: String): List<CatalogItem> {
  return try {
    val array = JSONArray(rawJson)
    buildList {
      for (index in 0 until array.length()) {
        val obj = array.optJSONObject(index) ?: continue
        val id = obj.optString("id").trim()
        val title = obj.optString("title").trim()
        if (id.isEmpty() || title.isEmpty()) {
          continue
        }
        val posterUrl = obj.optString("poster_url").trim().ifEmpty { null }
        add(CatalogItem(id = id, title = title, posterUrl = posterUrl))
      }
    }
  } catch (_: Throwable) {
    emptyList()
  }
}

private fun parseFirstStream(rawJson: String): StreamInfo? {
  return try {
    val root = JSONObject(rawJson)
    val streams = root.optJSONArray("streams") ?: return null
    for (index in 0 until streams.length()) {
      val stream = streams.optJSONObject(index) ?: continue
      val url = stream.optString("url").trim()
      if (url.isEmpty()) {
        continue
      }
      val name = stream.optString("name").trim().ifEmpty { "Stream" }
      return StreamInfo(name = name, url = url)
    }
    null
  } catch (_: Throwable) {
    null
  }
}
