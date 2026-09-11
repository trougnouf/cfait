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
import androidx.compose.runtime.mutableIntStateOf
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
 * search), per-mode settings, a custom label, and a text color that stays
 * readable against their wallpaper.
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
            Color.White to "white",
            Color.Black to "black",
            Color(0xFFFFCC00) to "yellow",
            Color(0xFFFF5500) to "orange",
            Color(0xFF4FC3F7) to "blue",
            Color(0xFFB388FF) to "purple",
            Color(0xFF69F0AE) to "mint",
            Color(0xFFFF80AB) to "pink",
            Color(0xFFB0BEC5) to "grey",
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
                mutableIntStateOf(prefs.getInt(KEY_TEXT_COLOR_INDEX + s, 0))
            }
            var customColor by remember {
                mutableIntStateOf(prefs.getInt(KEY_CUSTOM_COLOR + s, -1))
            }
            var useCollectionColor by remember {
                mutableStateOf(prefs.getBoolean(KEY_USE_COLLECTION_COLOR + s, false))
            }
            var customLabel by remember {
                mutableStateOf(prefs.getString(KEY_CUSTOM_LABEL + s, "") ?: "")
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

                        if (mode != 2) {
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

                        Text("Label", style = MaterialTheme.typography.labelLarge)
                        OutlinedTextField(
                            value = customLabel,
                            onValueChange = { customLabel = it },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            placeholder = { Text(modeLabels[mode]) }
                        )

                        Spacer(modifier = Modifier.height(8.dp))

                        Text("Text color", style = MaterialTheme.typography.labelLarge)
                        Row(
                            modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            textColors.forEachIndexed { index, (color, _label) ->
                                val selected = !useCollectionColor && customColor < 0 && index == textColorIndex
                                Box(
                                    modifier = Modifier
                                        .size(32.dp)
                                        .background(color = color, shape = CircleShape)
                                        .then(
                                            if (selected)
                                                Modifier.border(2.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                            else Modifier
                                        )
                                        .clickable {
                                            textColorIndex = index
                                            customColor = -1
                                            useCollectionColor = false
                                        }
                                )
                            }
                            // Custom color swatch — tap to cycle through hues
                            val customSelected = !useCollectionColor && customColor >= 0
                            Box(
                                modifier = Modifier
                                    .size(32.dp)
                                    .background(
                                        color = if (customColor >= 0) Color(customColor) else MaterialTheme.colorScheme.surfaceVariant,
                                        shape = CircleShape
                                    )
                                    .then(
                                        if (customSelected)
                                            Modifier.border(2.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                        else Modifier
                                    )
                                    .clickable {
                                        val hue = ((customColor + 30) % 360).coerceAtLeast(0)
                                        customColor = android.graphics.Color.HSVToColor(
                                            floatArrayOf(hue.toFloat(), 0.8f, 1.0f)
                                        )
                                        useCollectionColor = false
                                    }
                            )
                            // Collection color swatch
                            if (mode != 2 && !selectedCalHref.isNullOrEmpty()) {
                                val calColor = calendars.firstOrNull { it.href == selectedCalHref }?.color
                                    ?.let { hex ->
                                        try { Color(android.graphics.Color.parseColor(hex)) } catch (_: Exception) { null }
                                    }
                                if (calColor != null) {
                                    Box(
                                        modifier = Modifier
                                            .size(32.dp)
                                            .background(color = calColor, shape = CircleShape)
                                            .then(
                                                if (useCollectionColor)
                                                    Modifier.border(2.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                                else Modifier
                                            )
                                            .clickable {
                                                useCollectionColor = true
                                                customColor = -1
                                            }
                                    )
                                }
                            }
                        }

                        Button(
                            onClick = {
                                val finalColor = when {
                                    useCollectionColor -> {
                                        val cal = calendars.firstOrNull { it.href == selectedCalHref }
                                        cal?.color?.let { hex ->
                                            try { android.graphics.Color.parseColor(hex) } catch (_: Exception) { 0xFFFFFFFF.toInt() }
                                        } ?: 0xFFFFFFFF.toInt()
                                    }
                                    customColor >= 0 -> customColor
                                    else -> textColors[textColorIndex].first.toArgb()
                                }
                                prefs.edit()
                                    .putInt(KEY_MODE + s, mode)
                                    .putString(KEY_CALENDAR_HREF + s, selectedCalHref)
                                    .putString(KEY_SEARCH_QUERY + s, searchQuery.ifBlank { "is:ready" })
                                    .putInt(KEY_TEXT_COLOR + s, finalColor)
                                    .putInt(KEY_TEXT_COLOR_INDEX + s, textColorIndex)
                                    .putInt(KEY_CUSTOM_COLOR + s, customColor)
                                    .putBoolean(KEY_USE_COLLECTION_COLOR + s, useCollectionColor)
                                    .putString(KEY_CUSTOM_LABEL + s, customLabel)
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
        const val KEY_CUSTOM_COLOR = "custom_color"
        const val KEY_USE_COLLECTION_COLOR = "use_collection_color"
        const val KEY_CUSTOM_LABEL = "custom_label"
    }
}
