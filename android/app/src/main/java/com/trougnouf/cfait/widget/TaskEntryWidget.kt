// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.trougnouf.cfait.MainActivity

/** Intent extra that asks MainActivity to focus the new-task input. */
const val EXTRA_QUICK_ADD = "quick_add"

private val QuickAddKey = ActionParameters.Key<String>(EXTRA_QUICK_ADD)

/**
 * A compact home-screen "add task" button widget.
 *
 * Android widgets cannot host an editable text field, so this widget is a
 * single tappable button that launches MainActivity focused on the new-task
 * input (which does have the app's smart-string syntax highlighting).
 */
class TaskEntryWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val prefs = context.getSharedPreferences(
            TaskEntryWidgetConfigActivity.PREFS_NAME,
            Context.MODE_PRIVATE
        )
        val textColorArgb = prefs.getInt(TaskEntryWidgetConfigActivity.KEY_TEXT_COLOR, 0xFFFFFFFF.toInt())
        val textColor = Color(textColorArgb)

        provideContent {
            GlanceTheme(colors = WidgetColorScheme) {
                Box(
                    modifier = GlanceModifier
                        .fillMaxSize()
                        .clickable(
                            actionStartActivity(
                                MainActivity::class.java,
                                actionParametersOf(QuickAddKey to "1")
                            )
                        ),
                    contentAlignment = Alignment.BottomCenter
                ) {
                    Image(
                        provider = ImageProvider(com.trougnouf.cfait.R.drawable.ic_launcher_foreground),
                        contentDescription = "Add task",
                        modifier = GlanceModifier.fillMaxSize().padding(bottom = 10.dp),
                    )
                    Text(
                        text = "Add task",
                        style = TextStyle(
                            fontSize = 10.sp,
                            color = ColorProvider(textColor),
                        )
                    )
                }
            }
        }
    }
}

class TaskEntryWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget = TaskEntryWidget()
}
