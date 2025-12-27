package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.cfait.core.CfaitMobile
import com.cfait.util.AlarmScheduler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            val api = CfaitMobile(context.filesDir.absolutePath)
            CoroutineScope(Dispatchers.IO).launch {
                AlarmScheduler.scheduleNextAlarm(context, api)
            }
        }
    }
}
