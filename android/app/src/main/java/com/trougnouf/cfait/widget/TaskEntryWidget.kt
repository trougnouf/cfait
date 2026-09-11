// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import androidx.glance.currentState
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.state.PreferencesGlanceStateDefinition
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.R

/** Intent extra that asks MainActivity to focus the new-task input. */
const val EXTRA_QUICK_ADD = "quick_add"

/** Intent extra asking MainActivity to open the journal tab for today. */
const val EXTRA_JOURNAL_TODAY = "journal_today"

/** Intent extra with the calendar href for journal/search modes. */
const val EXTRA_CALENDAR_HREF = "widget_calendar_href"

/** Intent extra asking MainActivity to open with a preset search query. */
const val EXTRA_PRESET_SEARCH = "preset_search"

private val QuickAddKey = ActionParameters.Key<String>(EXTRA_QUICK_ADD)
private val JournalTodayKey = ActionParameters.Key<String>(EXTRA_JOURNAL_TODAY)
private val CalendarHrefKey = ActionParameters.Key<String>(EXTRA_CALENDAR_HREF)
private val PresetSearchKey = ActionParameters.Key<String>(EXTRA_PRESET_SEARCH)

private val MODE_QUICK_ADD = 0
private val MODE_JOURNAL = 1
private val MODE_SEARCH = 2

private val modeLabelsRes = mapOf(
    MODE_QUICK_ADD to R.string.widget_add_task,
    MODE_JOURNAL to R.string.journal,
    MODE_SEARCH to R.string.search,
)

/** State key used to force widget recomposition after config changes. */
val EntryWidgetRefreshTickKey = longPreferencesKey("refresh_tick")

/**
 * A compact home-screen widget that launches the app in one of three modes:
 * quick-add task (focus the new-task input), journal entry for today (with a
 * pre-set collection), or open with a pre-set search query.
 *
 * Android widgets cannot host an editable text field, so this widget is a
 * single tappable button. It shows the Cfait logo with a mode-specific label.
 */
class TaskEntryWidget : GlanceAppWidget() {

    override val stateDefinition = PreferencesGlanceStateDefinition

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val prefs = context.getSharedPreferences(
            TaskEntryWidgetConfigActivity.PREFS_NAME,
            Context.MODE_PRIVATE
        )
        val suffix = if (id is androidx.glance.appwidget.AppWidgetId) "_${id.appWidgetId}" else ""

        provideContent {
            // Reading the refresh tick from Glance state forces recomposition
            // when the config activity bumps it via updateAppWidgetState.
            currentState<Preferences>()[EntryWidgetRefreshTickKey]

            val mode = prefs.getInt(TaskEntryWidgetConfigActivity.KEY_MODE + suffix, 0)
            val calHref = prefs.getString(TaskEntryWidgetConfigActivity.KEY_CALENDAR_HREF + suffix, "") ?: ""
            val searchQuery = prefs.getString(TaskEntryWidgetConfigActivity.KEY_SEARCH_QUERY + suffix, "") ?: ""
            val textColorArgb = prefs.getInt(TaskEntryWidgetConfigActivity.KEY_TEXT_COLOR + suffix, 0xFFFFFFFF.toInt())
            val textColor = Color(textColorArgb)
            val customLabel = prefs.getString(TaskEntryWidgetConfigActivity.KEY_CUSTOM_LABEL + suffix, "") ?: ""
            val defaultLabel = context.getString(modeLabelsRes[mode] ?: R.string.widget_add_task)
            val label = customLabel.ifBlank { defaultLabel }

            val params = when (mode) {
                MODE_JOURNAL -> actionParametersOf(
                    JournalTodayKey to "1",
                    CalendarHrefKey to calHref
                )
                MODE_SEARCH -> actionParametersOf(
                    PresetSearchKey to searchQuery,
                    CalendarHrefKey to calHref
                )
                else -> actionParametersOf(
                    QuickAddKey to "1",
                    CalendarHrefKey to calHref
                )
            }

            GlanceTheme(colors = WidgetColorScheme) {
                Box(
                    modifier = GlanceModifier
                        .fillMaxSize()
                        .clickable(
                            actionStartActivity(MainActivity::class.java, params)
                        ),
                    contentAlignment = Alignment.BottomCenter
                ) {
                    Image(
                        provider = ImageProvider(com.trougnouf.cfait.R.drawable.ic_launcher_foreground),
                        contentDescription = label,
                        modifier = GlanceModifier.fillMaxSize().padding(bottom = 10.dp),
                    )
                    Text(
                        text = label,
                        style = TextStyle(
                            fontSize = 10.sp,
                            color = ColorProvider(textColor),
                        )
                    )
                }
            }
        }
    }

    override suspend fun onDelete(context: Context, glanceId: GlanceId) {
        if (glanceId is androidx.glance.appwidget.AppWidgetId) {
            val s = "_${glanceId.appWidgetId}"
            context.getSharedPreferences(TaskEntryWidgetConfigActivity.PREFS_NAME, Context.MODE_PRIVATE)
                .edit()
                .remove(TaskEntryWidgetConfigActivity.KEY_MODE + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_CALENDAR_HREF + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_SEARCH_QUERY + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_TEXT_COLOR + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_TEXT_COLOR_INDEX + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_CUSTOM_COLOR + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_USE_COLLECTION_COLOR + s)
                .remove(TaskEntryWidgetConfigActivity.KEY_CUSTOM_LABEL + s)
                .apply()
        }
    }
}

class TaskEntryWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget = TaskEntryWidget()

    private fun isOwnId(context: Context, appWidgetId: Int): Boolean {
        val info = AppWidgetManager.getInstance(context).getAppWidgetInfo(appWidgetId)
        val ownComponent = ComponentName(context.packageName, javaClass.name)
        val isOwn = info?.provider == ownComponent
        if (!isOwn) {
            android.util.Log.w("CfaitWidget", "EntryReceiver ignoring id=$appWidgetId provider=${info?.provider} (expected $ownComponent)")
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
