// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import org.json.JSONObject

/**
 * Reads the widget snapshot JSON file written by the Rust core's
 * `write_widget_snapshot` FFI method.
 *
 * The file is written atomically (temp-then-rename) so a cold widget
 * render never reads a partial file.
 */
object WidgetSnapshotStore {

    private const val FILE_NAME = "widget_snapshot.json"

    fun snapshotFile(context: Context): java.io.File =
        java.io.File(context.cacheDir, FILE_NAME)

    data class TaskItem(
        val uid: String,
        val summary: String,
        val isDone: Boolean,
        val isPaused: Boolean,
        val isBlocked: Boolean,
        val priority: Int,
        val dueDateIso: String?,
        val isDueToday: Boolean,
        val isAlldayDue: Boolean,
        val calendarColor: String?,
    )

    data class OngoingItem(
        val uid: String,
        val summary: String,
        val lastStartedAt: Long?,
    )

    data class Snapshot(
        val generatedAt: Long,
        val tasks: List<TaskItem>,
        val ongoing: List<OngoingItem>,
        val readyCount: Int,
        val overdueCount: Int,
        val ongoingCount: Int,
    )

    fun read(context: Context): Snapshot? {
        val file = snapshotFile(context)
        if (!file.exists()) return null
        return try {
            val json = JSONObject(file.readText())
            val tasks = json.optJSONArray("tasks")?.let { arr ->
                (0 until arr.length()).map { i ->
                    val o = arr.getJSONObject(i)
                    TaskItem(
                        uid = o.getString("uid"),
                        summary = o.getString("summary"),
                        isDone = o.optBoolean("is_done", false),
                        isPaused = o.optBoolean("is_paused", false),
                        isBlocked = o.optBoolean("is_blocked", false),
                        priority = o.optInt("priority", 5),
                        dueDateIso = o.optString("due_date_iso").takeIf { it.isNotBlank() },
                        isDueToday = o.optBoolean("is_due_today", false),
                        isAlldayDue = o.optBoolean("is_allday_due", false),
                        calendarColor = o.optString("calendar_color").takeIf { it.isNotBlank() },
                    )
                }
            } ?: emptyList()

            val ongoing = json.optJSONArray("ongoing")?.let { arr ->
                (0 until arr.length()).map { i ->
                    val o = arr.getJSONObject(i)
                    OngoingItem(
                        uid = o.getString("uid"),
                        summary = o.getString("summary"),
                        lastStartedAt = if (o.isNull("last_started_at")) null else o.optLong("last_started_at"),
                    )
                }
            } ?: emptyList()

            val counts = json.optJSONObject("counts")
            Snapshot(
                generatedAt = json.optLong("generated_at"),
                tasks = tasks,
                ongoing = ongoing,
                readyCount = counts?.optInt("ready", 0) ?: 0,
                overdueCount = counts?.optInt("overdue", 0) ?: 0,
                ongoingCount = counts?.optInt("ongoing", 0) ?: 0,
            )
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to read widget snapshot", e)
            null
        }
    }

    /**
     * Regenerate the snapshot via the Rust core, then update all widgets.
     * Uses the per-widget config (search query, filters) from SharedPreferences.
     */
    fun refresh(context: Context, api: com.trougnouf.cfait.core.CfaitMobile) {
        try {
            val prefs = context.getSharedPreferences("cfait_widget_prefs", Context.MODE_PRIVATE)
            val searchQuery = prefs.getString("search_query", "is:ready") ?: "is:ready"
            val maxTasks = prefs.getInt("max_tasks", 8)
            val hideChecked = prefs.getBoolean("hide_checked", false)
            val effectiveQuery = if (hideChecked && !searchQuery.contains("is:done")) {
                "$searchQuery -is:done"
            } else {
                searchQuery
            }

            api.writeWidgetSnapshot(
                searchQuery = effectiveQuery,
                filterTags = emptyList(),
                filterLocations = emptyList(),
                matchAllCategories = false,
                maxTasks = maxTasks.toUInt(),
                path = snapshotFile(context).absolutePath,
            )
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to write widget snapshot", e)
        }
    }
}
