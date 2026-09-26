// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import android.util.Log
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.state.updateAppWidgetState
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.core.AppIntent
import com.trougnouf.cfait.ui.triggerBackgroundSync
import com.trougnouf.cfait.util.NotificationHelper

/**
 * ActionCallback triggered when the user taps the fold/unfold indicator of a
 * task tree row in the widget. Toggles the collapsed state via the live API so
 * the widget mirrors the same fold/unfold status used in the main app.
 */
class ToggleTreeCollapseActionCallback : ActionCallback {

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
            app.api.dispatch(AppIntent.ToggleTreeCollapse(uid = uid))
            triggerBackgroundSync(context, app.api)
        } catch (e: Exception) {
            Log.w("CfaitWidget", "Failed to toggle tree collapse for $uid", e)
            NotificationHelper.showWidgetErrorNotification(context)
            return
        }

        updateAppWidgetState(context, glanceId) { prefs ->
            prefs[RefreshTickKey] = System.currentTimeMillis()
        }
        updateAllTaskListWidgets(context)
    }
}
