// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.appwidget.updateAll
import com.trougnouf.cfait.CfaitApplication

/**
 * ActionCallback triggered when the user taps a checkbox in the widget.
 * Toggles the task done/undone via the live API, then updates the Glance
 * state (a refresh tick) to force recomposition and calls updateAll.
 *
 * Glance only re-runs provideGlance when no session is running. When a
 * session IS running, calling update() recomposes using the existing
 * session but only if the Glance state has changed. By bumping a
 * timestamp in the state, we force recomposition every time.
 */
class ToggleTaskActionCallback : ActionCallback {

    companion object {
        val TaskUidKey = ActionParameters.Key<String>("task_uid")
        private val RefreshTickKey = longPreferencesKey("refresh_tick")
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
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to toggle task $uid", e)
            return
        }

        // Bump the refresh tick in Glance state to force recomposition.
        updateAppWidgetState(context, glanceId) { prefs ->
            prefs[RefreshTickKey] = System.currentTimeMillis()
        }
        TaskListWidget().updateAll(context)
    }
}
