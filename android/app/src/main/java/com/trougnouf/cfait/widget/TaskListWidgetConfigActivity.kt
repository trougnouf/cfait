// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.updateAll
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
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

        val prefs = getSharedPreferences("cfait_widget_prefs", Context.MODE_PRIVATE)

        setContent {
            var searchQuery by remember {
                mutableStateOf(prefs.getString("search_query", "is:ready") ?: "is:ready")
            }
            var hideChecked by remember {
                mutableStateOf(prefs.getBoolean("hide_checked", false))
            }
            var maxTasks by remember {
                mutableStateOf(prefs.getInt("max_tasks", 8).toString())
            }

            MaterialTheme {
                Scaffold(
                    topBar = {
                        TopAppBar(title = { Text("Cfait widget") })
                    }
                ) { padding ->
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(padding)
                            .padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        Text("Search query", style = MaterialTheme.typography.labelLarge)
                        OutlinedTextField(
                            value = searchQuery,
                            onValueChange = { searchQuery = it },
                            modifier = Modifier.fillMaxSize().height(56.dp),
                            singleLine = false,
                            placeholder = { Text("is:ready") }
                        )

                        Row(
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Checkbox(
                                checked = hideChecked,
                                onCheckedChange = { hideChecked = it }
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text("Hide checked tasks")
                        }

                        Row(
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

                        Row(
                            modifier = Modifier.fillMaxSize(),
                            horizontalArrangement = Arrangement.End
                        ) {
                            Button(onClick = {
                                val max = maxTasks.toIntOrNull()?.coerceIn(1, 20) ?: 8
                                prefs.edit()
                                    .putString("search_query", searchQuery.ifBlank { "is:ready" })
                                    .putBoolean("hide_checked", hideChecked)
                                    .putInt("max_tasks", max)
                                    .apply()

                                val resultValue = Intent().putExtra(
                                    AppWidgetManager.EXTRA_APPWIDGET_ID,
                                    appWidgetId
                                )
                                setResult(RESULT_OK, resultValue)

                                // Trigger an immediate snapshot + widget update
                                val app = applicationContext as com.trougnouf.cfait.CfaitApplication
                                WidgetSnapshotStore.refresh(this@TaskListWidgetConfigActivity, app.api)
                                CoroutineScope(Dispatchers.Default).launch {
                                    TaskListWidget().updateAll(this@TaskListWidgetConfigActivity)
                                }

                                finish()
                            }) {
                                Text("Add widget")
                            }
                        }
                    }
                }
            }
        }
    }
}
