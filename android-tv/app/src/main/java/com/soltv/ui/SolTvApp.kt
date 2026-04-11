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
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import coil.compose.AsyncImage
import com.soltv.bridge.RustBridge
import org.json.JSONArray
import org.json.JSONObject

@Composable
fun SolTvApp(nativeStatus: String, homeFeedJson: String, catalogJson: String) {
  val homeFeed = parseHomeFeedSnapshot(homeFeedJson)
  val catalog = parseCatalogSnapshot(catalogJson)

  var selectedItemForSources by remember { mutableStateOf<MediaCard?>(null) }
  var selectedStreams by remember { mutableStateOf<List<StreamInfo>>(emptyList()) }
  var playback by remember { mutableStateOf<PlaybackSelection?>(null) }
  var feedback by remember { mutableStateOf<String?>(null) }

  fun openItem(item: MediaCard) {
    val lookup = parseStreamLookup(RustBridge.streamsJsonOrFallback(item.id))
    val streams = lookup.streams
    if (streams.isEmpty()) {
      feedback = lookup.message.ifBlank { "No playable stream found for ${item.title}." }
      return
    }

    feedback = null
    if (streams.size == 1) {
      playback = PlaybackSelection(
        title = item.title,
        streamName = streams.first().name,
        streamUrl = streams.first().url,
      )
    } else {
      selectedItemForSources = item
      selectedStreams = streams
    }
  }

  MaterialTheme {
    Surface(modifier = Modifier.fillMaxSize()) {
      when {
        playback != null -> {
          VideoViewPlayerScreen(
            selection = playback!!,
            onBack = { playback = null },
          )
        }

        selectedItemForSources != null -> {
          SourcePickerScreen(
            item = selectedItemForSources!!,
            streams = selectedStreams,
            onBack = {
              selectedItemForSources = null
              selectedStreams = emptyList()
            },
            onSelectStream = { stream ->
              playback = PlaybackSelection(
                title = selectedItemForSources!!.title,
                streamName = stream.name,
                streamUrl = stream.url,
              )
            },
          )
        }

        else -> {
          LazyColumn(
            modifier = Modifier
              .fillMaxSize()
              .padding(horizontal = 40.dp, vertical = 28.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
          ) {
            item {
              Text(text = "Sol for Android TV", style = MaterialTheme.typography.headlineLarge)
            }
            item {
              Text(text = "Native bridge: $nativeStatus")
            }

            homeFeed.hero?.let { heroItem ->
              item {
                HeroBanner(item = heroItem, onClick = { openItem(heroItem) })
              }
            }

            if (homeFeed.continueWatching.isNotEmpty()) {
              item {
                Text(text = "Continue watching", style = MaterialTheme.typography.titleLarge)
              }
              item {
                MediaRail(items = homeFeed.continueWatching, onClick = { openItem(it) })
              }
            }

            if (homeFeed.trending.isNotEmpty()) {
              item {
                Text(text = "Trending", style = MaterialTheme.typography.titleLarge)
              }
              item {
                MediaRail(items = homeFeed.trending, onClick = { openItem(it) })
              }
            }

            if (catalog.isNotEmpty()) {
              item {
                Text(text = "Catalog", style = MaterialTheme.typography.titleLarge)
              }
              item {
                MediaRail(items = catalog, onClick = { openItem(it) })
              }
            }

            if (!feedback.isNullOrBlank()) {
              item {
                Text(text = feedback ?: "", color = Color(0xFFFFB4AB))
              }
            }
          }
        }
      }
    }
  }
}

@Composable
private fun HeroBanner(item: MediaCard, onClick: () -> Unit) {
  Box(
    modifier = Modifier
      .fillMaxWidth()
      .height(260.dp)
      .clip(RoundedCornerShape(18.dp))
      .background(Color(0xFF14283A))
      .clickable(
        interactionSource = remember { MutableInteractionSource() },
        indication = null,
      ) { onClick() },
  ) {
    val imageUrl = item.backdropUrl ?: item.posterUrl
    if (!imageUrl.isNullOrBlank()) {
      AsyncImage(
        model = imageUrl,
        contentDescription = "${item.title} hero",
        contentScale = ContentScale.Crop,
        modifier = Modifier.fillMaxSize(),
      )
    }

    Box(
      modifier = Modifier
        .fillMaxSize()
        .background(Color(0x8007131F)),
    )

    Box(
      modifier = Modifier
        .align(Alignment.BottomStart)
        .padding(18.dp),
    ) {
      Text(
        text = item.title,
        color = Color.White,
        style = MaterialTheme.typography.headlineMedium,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
      )
    }
  }
}

@Composable
private fun MediaRail(items: List<MediaCard>, onClick: (MediaCard) -> Unit) {
  LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
    items(items, key = { item -> item.id }) { item ->
      PosterCard(item = item, onClick = { onClick(item) })
    }
  }
}

@Composable
private fun SourcePickerScreen(
  item: MediaCard,
  streams: List<StreamInfo>,
  onBack: () -> Unit,
  onSelectStream: (StreamInfo) -> Unit,
) {
  BackHandler(onBack = onBack)

  LazyColumn(
    modifier = Modifier
      .fillMaxSize()
      .padding(horizontal = 40.dp, vertical = 28.dp),
    verticalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    item {
      Text(text = "Choose Source", style = MaterialTheme.typography.headlineLarge)
    }
    item {
      Text(text = item.title, style = MaterialTheme.typography.titleLarge)
    }
    item {
      Text(text = "Press Back to return")
    }

    items(streams, key = { stream -> "${stream.name}:${stream.url}" }) { stream ->
      SourceCard(stream = stream, onClick = { onSelectStream(stream) })
    }
  }
}

@Composable
private fun SourceCard(stream: StreamInfo, onClick: () -> Unit) {
  var isFocused by remember { mutableStateOf(false) }
  val scale by animateFloatAsState(targetValue = if (isFocused) 1.02f else 1.0f, label = "sourceScale")

  Box(
    modifier = Modifier
      .fillMaxWidth()
      .scale(scale)
      .clip(RoundedCornerShape(12.dp))
      .background(if (isFocused) Color(0xFF1E3A5F) else Color(0xFF102236))
      .onFocusChanged { state -> isFocused = state.isFocused }
      .focusable()
      .clickable(
        interactionSource = remember { MutableInteractionSource() },
        indication = null,
      ) { onClick() }
      .padding(horizontal = 14.dp, vertical = 12.dp),
  ) {
    Text(
      text = stream.name,
      color = Color.White,
      style = MaterialTheme.typography.bodyLarge,
      maxLines = 2,
      overflow = TextOverflow.Ellipsis,
    )
  }
}

@Composable
private fun VideoViewPlayerScreen(
  selection: PlaybackSelection,
  onBack: () -> Unit,
) {
  var playerError by remember { mutableStateOf<String?>(null) }

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
            playerError = null
            start()
          }

          setOnErrorListener { _, what, extra ->
            playerError = "what=$what extra=$extra"
            true
          }

          setVideoURI(Uri.parse(selection.streamUrl))
          requestFocus()
          start()
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

    if (!playerError.isNullOrBlank()) {
      Box(
        modifier = Modifier
          .align(Alignment.BottomStart)
          .padding(16.dp)
          .background(Color(0xB3000000), RoundedCornerShape(10.dp))
          .padding(horizontal = 12.dp, vertical = 8.dp),
      ) {
        Text(
          text = "Playback issue ($playerError). Press Back.",
          color = Color(0xFFFFB4AB),
        )
      }
    }
  }
}

@Composable
private fun PosterCard(item: MediaCard, onClick: () -> Unit) {
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
  val hero: MediaCard?,
  val trending: List<MediaCard>,
  val continueWatching: List<MediaCard>,
)

private data class MediaCard(
  val id: String,
  val title: String,
  val posterUrl: String?,
  val backdropUrl: String?,
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

private data class StreamLookupSnapshot(
  val message: String,
  val streams: List<StreamInfo>,
)

private fun parseHomeFeedSnapshot(rawJson: String): HomeFeedSnapshot {
  return try {
    val root = JSONObject(rawJson)
    val hero = root.optJSONObject("hero")?.let(::parseMediaCard)
    val trending = parseMediaArray(root.optJSONArray("trending"))
    val continueWatching = parseMediaArray(root.optJSONArray("continue_watching"))

    HomeFeedSnapshot(
      hero = hero,
      trending = trending,
      continueWatching = continueWatching,
    )
  } catch (_: Throwable) {
    HomeFeedSnapshot(hero = null, trending = emptyList(), continueWatching = emptyList())
  }
}

private fun parseCatalogSnapshot(rawJson: String): List<MediaCard> {
  return try {
    parseMediaArray(JSONArray(rawJson))
  } catch (_: Throwable) {
    emptyList()
  }
}

private fun parseMediaArray(array: JSONArray?): List<MediaCard> {
  if (array == null) {
    return emptyList()
  }

  return buildList {
    for (index in 0 until array.length()) {
      val obj = array.optJSONObject(index) ?: continue
      parseMediaCard(obj)?.let { add(it) }
    }
  }
}

private fun parseMediaCard(obj: JSONObject): MediaCard? {
  val id = obj.optString("id").trim()
  val title = obj.optString("title").trim()
  if (id.isEmpty() || title.isEmpty()) {
    return null
  }

  val posterUrl = obj.optString("poster_url").trim().ifEmpty { null }
  val backdropUrl = obj.optString("backdrop_url").trim().ifEmpty { null }

  return MediaCard(
    id = id,
    title = title,
    posterUrl = posterUrl,
    backdropUrl = backdropUrl,
  )
}

private fun parseStreamLookup(rawJson: String): StreamLookupSnapshot {
  return try {
    val root = JSONObject(rawJson)
    val message = root.optString("message").trim()
    val streamsArray = root.optJSONArray("streams") ?: JSONArray()
    val streams = buildList {
      for (index in 0 until streamsArray.length()) {
        val stream = streamsArray.optJSONObject(index) ?: continue
        val url = stream.optString("url").trim()
        if (!url.startsWith("http://") && !url.startsWith("https://")) {
          continue
        }
        val name = stream.optString("name").trim().ifEmpty { "Stream" }
        add(StreamInfo(name = name, url = url))
      }
    }

    StreamLookupSnapshot(message = message, streams = streams)
  } catch (_: Throwable) {
    StreamLookupSnapshot(message = "Could not parse stream lookup response.", streams = emptyList())
  }
}
