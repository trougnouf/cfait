package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationManagerCompat
import com.cfait.core.CfaitMobile
import com.cfait.util.AlarmScheduler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class NotificationActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val tUid = intent.getStringExtra("T_UID") ?: return
        val aUid = intent.getStringExtra("A_UID") ?: return
        val api = CfaitMobile(context.filesDir.absolutePath)

        // Close notification
        NotificationManagerCompat.from(context).cancel(aUid.hashCode())

        CoroutineScope(Dispatchers.IO).launch {
            try {
                if (intent.action == "SNOOZE") {
                    // Hardcoded 15m for notification action, could read config here too
                    api.snoozeAlarm(tUid, aUid, 15u)
                } else if (intent.action == "DISMISS") {
                    api.dismissAlarm(tUid, aUid)
                }

                // Recalculate schedule because the current alarm is now handled
                AlarmScheduler.scheduleNextAlarm(context, api)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
}
