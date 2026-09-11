// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.currentState
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.width
import androidx.glance.state.PreferencesGlanceStateDefinition
import androidx.glance.text.FontStyle
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextDecoration
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.core.MobileFilterOptions
import com.trougnouf.cfait.core.MobileTaskSummary

/** Intent extra key for deep-linking to a specific task from the widget. */
private val FocusTaskUidKey = ActionParameters.Key<String>("focus_task_uid")

/** State key used to force widget recomposition after in-widget actions or config changes. */
internal val RefreshTickKey = longPreferencesKey("refresh_tick")

/** Strip inline markdown markers (bold, italic, code, strikethrough) for plain display. */
private fun stripMarkdown(text: String): String {
    return text
        .replace(Regex("""\*\*(.+?)\*\*"""), "$1")
        .replace(Regex("""__(.+?)__"""), "$1")
        .replace(Regex("""~~(.+?)~~"""), "$1")
        .replace(Regex("""\*(.+?)\*"""), "$1")
        .replace(Regex("""_(.+?)_"""), "$1")
        .replace(Regex("""`(.+?)`"""), "$1")
}

class TaskListWidget : GlanceAppWidget() {

    override val stateDefinition = PreferencesGlanceStateDefinition

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val app = context.applicationContext as CfaitApplication
        val api = app.api

        val prefs = context.getSharedPreferences(TaskListWidgetConfigActivity.PREFS_NAME, Context.MODE_PRIVATE)
        val suffix = if (id is androidx.glance.appwidget.AppWidgetId) "_${id.appWidgetId}" else ""

        provideContent {
            // Read the refresh tick from Glance state. When the state
            // changes (after a toggle or config change), Glance recomposes
            // and this value updates, triggering a fresh data load.
            val refreshTick = currentState<Preferences>()[RefreshTickKey] ?: 0L

            // Re-read prefs on every recomposition so config changes take effect.
            val searchQuery = prefs.getString(TaskListWidgetConfigActivity.KEY_SEARCH_QUERY + suffix, "is:ready") ?: "is:ready"
            val maxTasks = prefs.getInt(TaskListWidgetConfigActivity.KEY_MAX_TASKS + suffix, 8)
            val hideChecked = prefs.getBoolean(TaskListWidgetConfigActivity.KEY_HIDE_CHECKED + suffix, false)
            val bgColorInt = prefs.getInt(TaskListWidgetConfigActivity.KEY_BG_COLOR + suffix, 0x80000000.toInt())
            val bgColor = Color(bgColorInt)
            val effectiveQuery = if (hideChecked && !searchQuery.contains("is:done")) {
                "$searchQuery -is:done"
            } else {
                searchQuery
            }
            val textColor = if ((bgColorInt ushr 24) > 0x80) {
                Color.Black
            } else {
                Color.White
            }

            // Hold the loaded data in state, keyed to refreshTick so it
            // reloads whenever the state changes.
            var viewData by remember { mutableStateOf<com.trougnouf.cfait.core.MobileViewData?>(null) }
            var calColorMap by remember { mutableStateOf<Map<String, Color>>(emptyMap()) }

            LaunchedEffect(refreshTick) {
                viewData = try {
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
                calColorMap = try {
                    api.getCalendars().associate { c ->
                        c.href to (c.color?.let { hex ->
                            try { Color(android.graphics.Color.parseColor(hex)) } catch (_: Exception) { Color.Gray }
                        } ?: Color.Gray)
                    }
                } catch (_: Exception) { emptyMap() }
            }

            GlanceTheme(colors = WidgetColorScheme) {
                Column(
                    modifier = GlanceModifier
                        .fillMaxWidth()
                        .background(bgColor)
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
                                color = ColorProvider(textColor),
                            )
                        )
                        Spacer(modifier = GlanceModifier.width(8.dp))
                        val vd = viewData
                        if (vd != null) {
                            val tasks = vd.tasks
                            val dueToday = tasks.count { it.isDueToday && !it.isDone }
                            val ongoing = tasks.count { it.isPaused }
                            if (dueToday > 0) {
                                Text(
                                    text = "$dueToday due today",
                                    style = TextStyle(
                                        fontSize = 13.sp,
                                        color = ColorProvider(textColor),
                                    )
                                )
                                Spacer(modifier = GlanceModifier.width(8.dp))
                            }
                            if (ongoing > 0) {
                                Text(
                                    text = "$ongoing active",
                                    style = TextStyle(
                                        fontSize = 13.sp,
                                        color = ColorProvider(textColor),
                                    )
                                )
                            }
                        }
                    }

                    Spacer(modifier = GlanceModifier.height(8.dp))

                    val vd = viewData
                    if (vd == null || vd.tasks.isEmpty()) {
                        Text(
                            text = if (vd == null) "Loading…" else "No tasks",
                            modifier = GlanceModifier.padding(top = 4.dp),
                            style = TextStyle(
                                fontSize = 14.sp,
                                color = ColorProvider(textColor),
                            )
                        )
                    } else {
                        vd.tasks.take(maxTasks).forEach { task ->
                            TaskRow(task, textColor, calColorMap[task.calendarHref] ?: Color.Gray)
                        }
                    }
                }
            }
        }
    }

    override suspend fun onDelete(context: Context, glanceId: GlanceId) {
        if (glanceId is androidx.glance.appwidget.AppWidgetId) {
            val s = "_${glanceId.appWidgetId}"
            context.getSharedPreferences(TaskListWidgetConfigActivity.PREFS_NAME, Context.MODE_PRIVATE)
                .edit()
                .remove(TaskListWidgetConfigActivity.KEY_SEARCH_QUERY + s)
                .remove(TaskListWidgetConfigActivity.KEY_HIDE_CHECKED + s)
                .remove(TaskListWidgetConfigActivity.KEY_MAX_TASKS + s)
                .remove(TaskListWidgetConfigActivity.KEY_BG_COLOR + s)
                .remove(TaskListWidgetConfigActivity.KEY_BG_COLOR_INDEX + s)
                .remove(TaskListWidgetConfigActivity.KEY_BG_OPACITY + s)
                .apply()
        }
    }
}

@androidx.compose.runtime.Composable
private fun TaskRow(task: MobileTaskSummary, textColor: Color, calColor: Color) {
    val indent = (task.depth.toInt() * 12).dp
    val displaySummary = stripMarkdown(task.summary)
    val isNote = task.isNote

    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .padding(start = indent, top = 3.dp, bottom = 3.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Collapse/expand indicator for tasks with subtasks
        if (task.hasSubtasks) {
            Text(
                text = if (task.isCollapsed) "▶" else "▼",
                style = TextStyle(
                    fontSize = 10.sp,
                    color = ColorProvider(textColor),
                )
            )
            Spacer(modifier = GlanceModifier.width(4.dp))
        } else {
            Spacer(modifier = GlanceModifier.width(14.dp))
        }

        // Checkbox (skip for notes)
        if (!isNote) {
            Text(
                text = if (task.isDone) "☑" else "☐",
                modifier = GlanceModifier.clickable(
                    actionRunCallback<ToggleTaskActionCallback>(
                        actionParametersOf(ToggleTaskActionCallback.TaskUidKey to task.uid)
                    )
                ),
                style = TextStyle(
                    fontSize = 16.sp,
                    color = ColorProvider(calColor),
                )
            )
            Spacer(modifier = GlanceModifier.width(6.dp))
        }

        // Title (tappable to open the app at this task)
        Column(
            modifier = GlanceModifier.clickable(
                actionStartActivity(
                    MainActivity::class.java,
                    actionParametersOf(FocusTaskUidKey to task.uid)
                )
            )
        ) {
            Text(
                text = displaySummary,
                maxLines = 1,
                style = TextStyle(
                    fontSize = 14.sp,
                    color = ColorProvider(textColor),
                    textDecoration = if (task.isDone) TextDecoration.LineThrough else TextDecoration.None,
                    fontStyle = if (isNote) FontStyle.Italic else FontStyle.Normal,
                )
            )
            if (task.dueDateIso != null && task.isDueToday) {
                Text(
                    text = "due today",
                    style = TextStyle(
                        fontSize = 12.sp,
                        color = ColorProvider(textColor.copy(alpha = 0.7f)),
                    )
                )
            }
        }
    }
}

class TaskListWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget = TaskListWidget()

    private fun isOwnId(context: Context, appWidgetId: Int): Boolean {
        val info = AppWidgetManager.getInstance(context).getAppWidgetInfo(appWidgetId)
        val ownComponent = ComponentName(context.packageName, javaClass.name)
        val isOwn = info?.provider == ownComponent
        if (!isOwn) {
            android.util.Log.w("CfaitWidget", "ListReceiver ignoring id=$appWidgetId provider=${info?.provider} (expected $ownComponent)")
        }
        return isOwn
    }

    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray
    ) {
        val ownIds = appWidgetIds.filter { isOwnId(context, it) }.toIntArray()
        if (ownIds.isNotEmpty()) super.onUpdate(context, appWidgetManager, ownIds)
    }

    override fun onAppWidgetOptionsChanged(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetId: Int,
        newOptions: android.os.Bundle
    ) {
        if (isOwnId(context, appWidgetId)) {
            super.onAppWidgetOptionsChanged(context, appWidgetManager, appWidgetId, newOptions)
        }
    }
}
