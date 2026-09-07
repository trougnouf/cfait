// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
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
import com.trougnouf.cfait.MainActivity

class TaskListWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val snapshot = WidgetSnapshotStore.read(context)

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
                        val counts = snapshot
                        if (counts != null && counts.overdueCount > 0) {
                            Text(
                                text = "${counts.overdueCount} overdue",
                                style = TextStyle(fontSize = 13.sp)
                            )
                            Spacer(modifier = GlanceModifier.width(8.dp))
                        }
                        if (counts != null && counts.ongoingCount > 0) {
                            Text(
                                text = "${counts.ongoingCount} active",
                                style = TextStyle(fontSize = 13.sp)
                            )
                        }
                    }

                    Spacer(modifier = GlanceModifier.height(8.dp))

                    if (snapshot == null || snapshot.tasks.isEmpty()) {
                        Text(
                            text = "No tasks",
                            modifier = GlanceModifier.padding(top = 4.dp),
                            style = TextStyle(fontSize = 14.sp)
                        )
                    } else {
                        snapshot.tasks.take(8).forEach { task ->
                            Row(
                                modifier = GlanceModifier
                                    .fillMaxWidth()
                                    .clickable(actionStartActivity(
                                        MainActivity::class.java
                                    ))
                                    .padding(vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                // Checkbox indicator
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
