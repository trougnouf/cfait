// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.core.MobileFilterOptions

/** Intent extra key for deep-linking to a specific task from the widget. */
private val FocusTaskUidKey = ActionParameters.Key<String>("focus_task_uid")

class TaskListWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val app = context.applicationContext as CfaitApplication
        val api = app.api

        val prefs = context.getSharedPreferences("cfait_widget_prefs", Context.MODE_PRIVATE)
        val searchQuery = prefs.getString("search_query", "is:ready") ?: "is:ready"
        val maxTasks = prefs.getInt("max_tasks", 8)
        val hideChecked = prefs.getBoolean("hide_checked", false)
        val effectiveQuery = if (hideChecked && !searchQuery.contains("is:done")) {
            "$searchQuery -is:done"
        } else {
            searchQuery
        }

        val viewData = try {
            api.getViewTasks(
                MobileFilterOptions(
                    filterTags = emptyList(),
                    filterLocations = emptyList(),
                    searchQuery = effectiveQuery,
                    expandedGroups = emptyList(),
                    matchAllCategories = false,
                    expandedTags = emptyList(),
                    expandedLocations = emptyList(),
                    offset = 0u,
                    limit = maxTasks.toUInt(),
                )
            )
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to load widget data", e)
            null
        }

        provideContent {
            GlanceTheme(colors = WidgetColorScheme) {
                Column(
                    modifier = GlanceModifier
                        .fillMaxWidth()
                        .padding(12.dp)
                ) {
                    // Header: app name + counts
                    Row(
                        modifier = GlanceModifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "Cfait",
                            style = TextStyle(
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold,
                            )
                        )
                        Spacer(modifier = GlanceModifier.width(8.dp))
                        if (viewData != null) {
                            val tasks = viewData.tasks
                            val overdue = tasks.count { it.isDueToday && !it.isDone }
                            val ongoing = tasks.count { it.isPaused }
                            if (overdue > 0) {
                                Text(
                                    text = "$overdue due today",
                                    style = TextStyle(fontSize = 13.sp)
                                )
                                Spacer(modifier = GlanceModifier.width(8.dp))
                            }
                            if (ongoing > 0) {
                                Text(
                                    text = "$ongoing active",
                                    style = TextStyle(fontSize = 13.sp)
                                )
                            }
                        }
                    }

                    Spacer(modifier = GlanceModifier.height(8.dp))

                    if (viewData == null || viewData.tasks.isEmpty()) {
                        Text(
                            text = if (viewData == null) "Loading…" else "No tasks",
                            modifier = GlanceModifier.padding(top = 4.dp),
                            style = TextStyle(fontSize = 14.sp)
                        )
                    } else {
                        viewData.tasks.take(maxTasks).forEach { task ->
                            Row(
                                modifier = GlanceModifier
                                    .fillMaxWidth()
                                    .clickable(actionStartActivity(
                                        MainActivity::class.java,
                                        actionParametersOf(FocusTaskUidKey to task.uid)
                                    ))
                                    .padding(vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = if (task.isDone) "[x]" else "[ ]",
                                    style = TextStyle(fontSize = 14.sp)
                                )
                                Spacer(modifier = GlanceModifier.width(6.dp))
                                Column {
                                    Text(
                                        text = task.summary,
                                        maxLines = 1,
                                        style = TextStyle(fontSize = 14.sp)
                                    )
                                    if (task.dueDateIso != null && task.isDueToday) {
                                        Text(
                                            text = "due today",
                                            style = TextStyle(fontSize = 12.sp)
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

class TaskListWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget = TaskListWidget()
}
