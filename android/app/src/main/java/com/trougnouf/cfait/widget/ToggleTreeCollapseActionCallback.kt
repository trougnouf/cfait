// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import android.content.Context
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.action.ActionParameters
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.state.updateAppWidgetState
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.core.AppIntent
import com.trougnouf.cfait.ui.triggerBackgroundSync

/**
 * ActionCallback triggered when the user taps the fold/unfold indicator of a
 * task tree row in the widget. Toggles the collapsed state via the live API so
 * the widget mirrors the same fold/unfold status used in the main app.
 */
class ToggleTreeCollapseActionCallback : ActionCallback {

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
            app.api.dispatch(AppIntent.ToggleTreeCollapse(uid = uid))
            triggerBackgroundSync(context, app.api)
        } catch (e: Exception) {
            android.util.Log.w("CfaitWidget", "Failed to toggle tree collapse for $uid", e)
            return
        }

        updateAppWidgetState(context, glanceId) { prefs ->
            prefs[RefreshTickKey] = System.currentTimeMillis()
        }
        updateAllTaskListWidgets(context)
    }
}
