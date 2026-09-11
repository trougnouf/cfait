// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch

/**
 * Configuration activity shown when the user places the task-list widget.
 * Lets them pick a search query, hide checked tasks, and set a max task count.
 */
class TaskListWidgetConfigActivity : ComponentActivity() {

    private var appWidgetId = AppWidgetManager.INVALID_APPWIDGET_ID

    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setResult(RESULT_CANCELED)

        appWidgetId = intent
            ?.extras
            ?.getInt(
                AppWidgetManager.EXTRA_APPWIDGET_ID,
                AppWidgetManager.INVALID_APPWIDGET_ID
            ) ?: AppWidgetManager.INVALID_APPWIDGET_ID

        if (appWidgetId == AppWidgetManager.INVALID_APPWIDGET_ID) {
            finish()
            return
        }

        val prefs = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val s = "_$appWidgetId"

        setContent {
            var searchQuery by remember {
                mutableStateOf(prefs.getString(KEY_SEARCH_QUERY + s, "is:ready") ?: "is:ready")
            }
            var hideChecked by remember {
                mutableStateOf(prefs.getBoolean(KEY_HIDE_CHECKED + s, false))
            }
            var maxTasks by remember {
                mutableStateOf(prefs.getInt(KEY_MAX_TASKS + s, 8).toString())
            }
            val bgColors = listOf(
                Color.Black to "Black",
                Color(0xFF1C1B1F) to "Dark",
                Color(0xFFE6E1E5) to "Light",
                Color(0xFF4E3390) to "Purple",
                Color(0xFF1B5E20) to "Green",
                Color(0xFF8C1D18) to "Red",
            )
            var bgColorIndex by remember {
                mutableStateOf(prefs.getInt(KEY_BG_COLOR_INDEX + s, 0))
            }
            var bgOpacity by remember {
                mutableFloatStateOf(prefs.getFloat(KEY_BG_OPACITY + s, 0.5f))
            }
            var respectCollapse by remember {
                mutableStateOf(prefs.getBoolean(KEY_RESPECT_COLLAPSE + s, true))
            }

            MaterialTheme {
                Scaffold(
                    topBar = {
                        TopAppBar(title = { Text("Cfait widget") })
                    }
                ) { padding ->
                    Column(
                        modifier = Modifier
                            .padding(padding)
                            .padding(16.dp)
                            .verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        Text("Search query", style = MaterialTheme.typography.labelLarge)
                        OutlinedTextField(
                            value = searchQuery,
                            onValueChange = { searchQuery = it },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            placeholder = { Text("is:ready") }
                        )

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Hide checked tasks")
                            Switch(
                                checked = hideChecked,
                                onCheckedChange = { hideChecked = it }
                            )
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Respect fold/unfold state")
                            Switch(
                                checked = respectCollapse,
                                onCheckedChange = { respectCollapse = it }
                            )
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Max tasks: ")
                            Spacer(modifier = Modifier.width(8.dp))
                            OutlinedTextField(
                                value = maxTasks,
                                onValueChange = { maxTasks = it.filter { c -> c.isDigit() } },
                                modifier = Modifier.width(80.dp),
                                singleLine = true
                            )
                        }

                        Spacer(modifier = Modifier.height(8.dp))

                        Text("Background", style = MaterialTheme.typography.labelLarge)
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            bgColors.forEachIndexed { index, (color, label) ->
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(32.dp)
                                            .background(
                                                color = color.copy(alpha = bgOpacity),
                                                shape = CircleShape
                                            )
                                            .then(
                                                if (index == bgColorIndex)
                                                    Modifier.border(2.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                                else Modifier
                                            )
                                            .clickable { bgColorIndex = index }
                                    )
                                    Text(label, style = MaterialTheme.typography.labelSmall)
                                }
                            }
                        }

                        Text("Opacity: ${(bgOpacity * 100).toInt()}%")
                        Slider(
                            value = bgOpacity,
                            onValueChange = { bgOpacity = it },
                            valueRange = 0f..1f,
                        )

                        Spacer(modifier = Modifier.height(8.dp))

                        Button(
                            onClick = {
                                val max = maxTasks.toIntOrNull()?.coerceIn(1, 20) ?: 8
                                val bgColor = bgColors[bgColorIndex].first
                                val bgColorArgb = (bgColor.copy(alpha = bgOpacity)).toArgb()
                                prefs.edit()
                                    .putString(KEY_SEARCH_QUERY + s, searchQuery.ifBlank { "is:ready" })
                                    .putBoolean(KEY_HIDE_CHECKED + s, hideChecked)
                                    .putBoolean(KEY_RESPECT_COLLAPSE + s, respectCollapse)
                                    .putInt(KEY_MAX_TASKS + s, max)
                                    .putInt(KEY_BG_COLOR + s, bgColorArgb)
                                    .putInt(KEY_BG_COLOR_INDEX + s, bgColorIndex)
                                    .putFloat(KEY_BG_OPACITY + s, bgOpacity)
                                    .apply()

                                val resultValue = Intent().putExtra(
                                    AppWidgetManager.EXTRA_APPWIDGET_ID,
                                    appWidgetId
                                )
                                setResult(RESULT_OK, resultValue)

                                lifecycleScope.launch {
                                    try {
                                        val manager = GlanceAppWidgetManager(this@TaskListWidgetConfigActivity)
                                        val glanceId = manager.getGlanceIdBy(appWidgetId)
                                        // Bump the refresh tick so the running Glance session
                                        // recomposes and re-reads the updated SharedPreferences.
                                        updateAppWidgetState(this@TaskListWidgetConfigActivity, glanceId) {
                                            it[RefreshTickKey] = System.currentTimeMillis()
                                        }
                                        TaskListWidget().update(this@TaskListWidgetConfigActivity, glanceId)
                                    } catch (e: Exception) {
                                        android.util.Log.w("CfaitWidget", "Widget update after config failed", e)
                                    } finally {
                                        finish()
                                    }
                                }
                            },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Text("Add widget")
                        }
                    }
                }
            }
        }
    }

    companion object {
        const val PREFS_NAME = "cfait_widget_prefs"
        const val KEY_SEARCH_QUERY = "search_query"
        const val KEY_HIDE_CHECKED = "hide_checked"
        const val KEY_MAX_TASKS = "max_tasks"
        const val KEY_BG_COLOR = "bg_color"
        const val KEY_BG_COLOR_INDEX = "bg_color_index"
        const val KEY_BG_OPACITY = "bg_opacity"
        const val KEY_RESPECT_COLLAPSE = "respect_collapse"
    }
}
