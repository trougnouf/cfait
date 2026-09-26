// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import android.util.Log
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.state.updateAppWidgetState
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.ui.triggerBackgroundSync
import com.trougnouf.cfait.util.NotificationHelper

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

    override suspend fun onAction(
        context: Context,
        glanceId: GlanceId,
        parameters: ActionParameters
    ) {
        val uid = parameters[TaskUidKey] ?: return
        try {
            val app = context.applicationContext as CfaitApplication
            // Wait for the background cache load before touching the store
            app.dataLoaded.await()
            app.api.toggleTask(uid)
            triggerBackgroundSync(context, app.api)
        } catch (e: Exception) {
            Log.w("CfaitWidget", "Failed to toggle task $uid", e)
            NotificationHelper.showWidgetErrorNotification(context)
            return
        }

        // Bump the refresh tick in Glance state to force recomposition.
        updateAppWidgetState(context, glanceId) { prefs ->
            prefs[RefreshTickKey] = System.currentTimeMillis()
        }
        updateAllTaskListWidgets(context)
    }
}
