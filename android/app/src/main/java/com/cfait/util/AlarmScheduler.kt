// Utility for scheduling Android AlarmManager events.
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
        Log.d("CfaitAlarm", "scheduleNextAlarm() called")

        val nextTs = api.getNextAlarmTimestamp()
        if (nextTs == null) {
            Log.d("CfaitAlarm", "No next alarm timestamp from Rust - nothing to schedule")
            return
        }

        val triggerMs = nextTs * 1000L
        val now = System.currentTimeMillis()
        val delaySeconds = (triggerMs - now) / 1000

        Log.d("CfaitAlarm", "Next alarm timestamp: $nextTs (Unix seconds)")
        Log.d("CfaitAlarm", "Trigger time: $triggerMs ms, Current time: $now ms")
        Log.d("CfaitAlarm", "Delay: $delaySeconds seconds from now")

        // Don't schedule in the past
        if (triggerMs <= now) {
            Log.w("CfaitAlarm", "Alarm time is in the past - not scheduling (delay: $delaySeconds seconds)")
            return
        }

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
            Log.i("CfaitAlarm", "✓ Successfully scheduled alarm for $triggerMs (in $delaySeconds seconds)")
        } catch (e: SecurityException) {
            Log.e("CfaitAlarm", "✗ SecurityException while scheduling alarm", e)
            e.printStackTrace()
        } catch (e: Exception) {
            Log.e("CfaitAlarm", "✗ Unexpected exception while scheduling alarm", e)
            e.printStackTrace()
        }
    }
}
