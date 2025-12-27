package com.cfait.util

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import com.cfait.core.CfaitMobile
import com.cfait.receivers.AlarmReceiver

object AlarmScheduler {
    fun scheduleNextAlarm(context: Context, api: CfaitMobile) {
        val nextTs = api.getNextAlarmTimestamp() ?: return
        val triggerMs = nextTs * 1000L
        val now = System.currentTimeMillis()

        // Don't schedule in the past
        if (triggerMs <= now) return

        val alarmManager = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val intent = Intent(context, AlarmReceiver::class.java)

        // FLAG_UPDATE_CURRENT ensures we replace any existing pending alarm
        val pendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                if (alarmManager.canScheduleExactAlarms()) {
                    alarmManager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerMs, pendingIntent)
                } else {
                    // Fallback: This is less precise but won't crash
                    alarmManager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerMs, pendingIntent)
                }
            } else {
                alarmManager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerMs, pendingIntent)
            }
            Log.d("CfaitAlarm", "Scheduled alarm for $triggerMs")
        } catch (e: SecurityException) {
            e.printStackTrace()
        }
    }
}
