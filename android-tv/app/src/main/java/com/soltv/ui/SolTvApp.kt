package com.soltv.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun SolTvApp(nativeStatus: String, homeFeedJson: String) {
  val feed = parseHomeFeedSnapshot(homeFeedJson)

  MaterialTheme {
    Surface(modifier = Modifier.fillMaxSize()) {
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
      }
    }
  }
}

private data class HomeFeedSnapshot(
  val heroTitle: String,
  val trendingTitles: List<String>,
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
