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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
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

/**
 * Configuration activity shown when the user places the task-entry widget.
 * Lets them pick a text color that stays readable against their wallpaper.
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

        val textColors = listOf(
            Color.White to "White",
            Color.Black to "Black",
            Color(0xFFFFCC00) to "Yellow",
            Color(0xFFFF5500) to "Orange",
            Color(0xFF4FC3F7) to "Blue",
            Color(0xFFB388FF) to "Purple",
        )

        setContent {
            var textColorIndex by remember {
                mutableStateOf(prefs.getInt(KEY_TEXT_COLOR_INDEX, 0))
            }

            MaterialTheme {
                Scaffold(
                    topBar = {
                        TopAppBar(title = { Text("Cfait quick-add widget") })
                    }
                ) { padding ->
                    Column(
                        modifier = Modifier
                            .padding(padding)
                            .padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        Text("Text color", style = MaterialTheme.typography.labelLarge)
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            textColors.forEachIndexed { index, (color, label) ->
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally
                                ) {
                                    androidx.compose.foundation.layout.Box(
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
                                    .putInt(KEY_TEXT_COLOR, colorArgb)
                                    .putInt(KEY_TEXT_COLOR_INDEX, textColorIndex)
                                    .apply()

                                val resultValue = Intent().putExtra(
                                    AppWidgetManager.EXTRA_APPWIDGET_ID,
                                    appWidgetId
                                )
                                setResult(RESULT_OK, resultValue)
                                finish()
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
        const val KEY_TEXT_COLOR = "text_color"
        const val KEY_TEXT_COLOR_INDEX = "text_color_index"
    }
}
