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
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.getValue
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
import com.trougnouf.cfait.CfaitApplication
import kotlinx.coroutines.launch

/**
 * Configuration activity shown when the user places the task-entry widget.
 * Lets them pick a mode (quick-add task, journal entry today, or open with
 * search), per-mode settings, and a text color that stays readable against
 * their wallpaper.
 */
class TaskEntryWidgetConfigActivity : ComponentActivity() {

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
        val api = (applicationContext as CfaitApplication).api
        val calendars = api.getCalendars().filter { !it.isDisabled }
        val s = "_$appWidgetId"

        val textColors = listOf(
            Color.White to "White",
            Color.Black to "Black",
            Color(0xFFFFCC00) to "Yellow",
            Color(0xFFFF5500) to "Orange",
            Color(0xFF4FC3F7) to "Blue",
            Color(0xFFB388FF) to "Purple",
        )

        setContent {
            var mode by remember { mutableStateOf(prefs.getInt(KEY_MODE + s, 0)) }
            var selectedCalHref by remember {
                mutableStateOf(prefs.getString(KEY_CALENDAR_HREF + s, calendars.firstOrNull()?.href ?: ""))
            }
            var searchQuery by remember {
                mutableStateOf(prefs.getString(KEY_SEARCH_QUERY + s, "is:ready") ?: "is:ready")
            }
            var textColorIndex by remember {
                mutableStateOf(prefs.getInt(KEY_TEXT_COLOR_INDEX + s, 0))
            }

            val modeLabels = listOf("Add task", "Journal", "Search")

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
                        Text("Mode", style = MaterialTheme.typography.labelLarge)
                        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                            modeLabels.forEachIndexed { index, label ->
                                SegmentedButton(
                                    selected = mode == index,
                                    onClick = { mode = index },
                                    shape = SegmentedButtonDefaults.itemShape(index, modeLabels.size)
                                ) {
                                    Text(label)
                                }
                            }
                        }

                        if (mode == 1 || mode == 2) {
                            Text("Collection", style = MaterialTheme.typography.labelLarge)
                            Row(
                                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                                horizontalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                calendars.forEach { cal ->
                                    FilterChip(
                                        selected = cal.href == selectedCalHref,
                                        onClick = { selectedCalHref = cal.href },
                                        label = { Text(cal.name) }
                                    )
                                }
                            }
                        }

                        if (mode == 2) {
                            Text("Search query", style = MaterialTheme.typography.labelLarge)
                            OutlinedTextField(
                                value = searchQuery,
                                onValueChange = { searchQuery = it },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                placeholder = { Text("is:ready") }
                            )
                        }

                        Spacer(modifier = Modifier.height(8.dp))

                        Text("Text color", style = MaterialTheme.typography.labelLarge)
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            textColors.forEachIndexed { index, (color, label) ->
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(32.dp)
                                            .background(color = color, shape = CircleShape)
                                            .then(
                                                if (index == textColorIndex)
                                                    Modifier.border(2.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                                else Modifier
                                            )
                                            .clickable { textColorIndex = index }
                                    )
                                    Text(label, style = MaterialTheme.typography.labelSmall)
                                }
                            }
                        }

                        Button(
                            onClick = {
                                val colorArgb = textColors[textColorIndex].first.toArgb()
                                prefs.edit()
                                    .putInt(KEY_MODE + s, mode)
                                    .putString(KEY_CALENDAR_HREF + s, selectedCalHref)
                                    .putString(KEY_SEARCH_QUERY + s, searchQuery.ifBlank { "is:ready" })
                                    .putInt(KEY_TEXT_COLOR + s, colorArgb)
                                    .putInt(KEY_TEXT_COLOR_INDEX + s, textColorIndex)
                                    .apply()

                                val resultValue = Intent().putExtra(
                                    AppWidgetManager.EXTRA_APPWIDGET_ID,
                                    appWidgetId
                                )
                                setResult(RESULT_OK, resultValue)

                                lifecycleScope.launch {
                                    try {
                                        val manager = GlanceAppWidgetManager(this@TaskEntryWidgetConfigActivity)
                                        val glanceId = manager.getGlanceIdBy(appWidgetId)
                                        // Bump the refresh tick so the running Glance session
                                        // recomposes and re-reads the updated SharedPreferences.
                                        updateAppWidgetState(this@TaskEntryWidgetConfigActivity, glanceId) {
                                            it[EntryWidgetRefreshTickKey] = System.currentTimeMillis()
                                        }
                                        TaskEntryWidget().update(this@TaskEntryWidgetConfigActivity, glanceId)
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
        const val PREFS_NAME = "cfait_entry_widget_prefs"
        const val KEY_MODE = "mode"
        const val KEY_CALENDAR_HREF = "calendar_href"
        const val KEY_SEARCH_QUERY = "search_query"
        const val KEY_TEXT_COLOR = "text_color"
        const val KEY_TEXT_COLOR_INDEX = "text_color_index"
    }
}
