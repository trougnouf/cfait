// SPDX-License-Identifier: GPL-3.0-or-later
// Receives system geofence transitions (LocationManager.addProximityAlert)
// and turns them into reminder notifications via the Rust evaluator.
package com.trougnouf.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.location.LocationManager
import android.util.Log
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.GeofenceRegistrar
import com.trougnouf.cfait.util.NotificationHelper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class ProximityAlertReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        val key = intent.getStringExtra(GeofenceRegistrar.EXTRA_GEOFENCE_KEY) ?: return
        val entering = intent.getBooleanExtra(LocationManager.KEY_PROXIMITY_ENTERING, true)
        val parts = key.split("|")
        val name = parts.getOrNull(0) ?: return
        val radius = parts.getOrNull(2)?.toUIntOrNull() ?: return

        val pendingResult = goAsync()
        CoroutineScope(Dispatchers.IO + SupervisorJob()).launch {
            try {
                val app = context.applicationContext as CfaitApplication
                app.dataLoaded.await()
                val api = app.api

                // Just-in-time sync: don't notify for tasks completed elsewhere.
                try {
                    api.sync()
                } catch (e: Exception) {
                    Log.w("CfaitProximityAlert", "Pre-alert sync failed: ${e.message}")
                }

                val fired = api.evaluateLocationAlert(name, radius, entering)
                if (fired.isNotEmpty()) {
                    Log.i("CfaitProximityAlert", "Geofence '$name' ($radius m, entering=$entering): ${fired.size} reminder(s) fired")
                    val prefs = context.getSharedPreferences("cfait_fired_alarms", Context.MODE_PRIVATE)
                    val recentlyFired = prefs.getStringSet("fired_uids", emptySet()) ?: emptySet()
                    val newFired = mutableSetOf<String>()
                    for (alarm in fired) {
                        newFired.add(alarm.alarmUid)
                        NotificationHelper.showAlarmNotification(
                            context,
                            alarm.title,
                            alarm.body,
                            alarm.taskUid,
                            alarm.alarmUid,
                            !recentlyFired.contains(alarm.alarmUid)
                        )
                    }
                    prefs.edit().putStringSet("fired_uids", newFired).apply()
                }
            } catch (e: Exception) {
                Log.e("CfaitProximityAlert", "Error handling proximity alert", e)
            } finally {
                pendingResult.finish()
            }
        }
    }
}
