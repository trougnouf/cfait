// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.action.ActionCallback
import com.trougnouf.cfait.CfaitApplication

/**
 * ActionCallback triggered when the user taps a checkbox in the widget.
 * Toggles the task done/undone via the live API and refreshes the widget.
 */
class ToggleTaskActionCallback : ActionCallback {

    companion object {
        val TaskUidKey = ActionParameters.Key<String>("task_uid")
    }

    override suspend fun onAction(
        context: Context,
        glanceId: GlanceId,
        parameters: ActionParameters
    ) {
        val uid = parameters[TaskUidKey] ?: return
        try {
            val app = context.applicationContext as CfaitApplication
            app.api.toggleTask(uid)
            // Update the specific widget that was tapped, not all of them.
            TaskListWidget().update(context, glanceId)
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to toggle task $uid", e)
        }
    }
}
