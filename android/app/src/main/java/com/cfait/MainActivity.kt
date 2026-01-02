// File: android/app/src/main/java/com/cfait/MainActivity.kt
package com.cfait

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.*
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import com.cfait.core.MobileCalendar
import com.cfait.core.MobileLocation
import com.cfait.core.MobileTag
import com.cfait.ui.HelpScreen
import com.cfait.ui.HomeScreen
import com.cfait.ui.SettingsScreen
import com.cfait.ui.TaskDetailScreen
import com.cfait.util.AlarmScheduler
import com.cfait.workers.AlarmWorker
import com.cfait.workers.CalendarSyncWorker
import com.cfait.workers.CalendarMigrationWorker
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val app = application as CfaitApplication
        val api = app.api

        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                CfaitNavHost(api)
            }
        }
    }
}

@Composable
fun CfaitNavHost(api: com.cfait.core.CfaitMobile) {
    val navController = rememberNavController()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    // Data State
    var calendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var tags by remember { mutableStateOf<List<MobileTag>>(emptyList()) }
    var locations by remember { mutableStateOf<List<MobileLocation>>(emptyList()) }
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    var hasUnsynced by remember { mutableStateOf(false) }
    var autoScrollUid by remember { mutableStateOf<String?>(null) }
    var refreshTick by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var isLoading by remember { mutableStateOf(false) }

    // --- WORK MANAGER OBSERVATION ---
    val workManager = WorkManager.getInstance(context)

    // Observe the specific unique work we queue for calendar sync
    val calendarWorkInfo by workManager
        .getWorkInfosForUniqueWorkLiveData(CalendarSyncWorker.UNIQUE_WORK_NAME)
        .observeAsState()

    val currentWorkInfo = calendarWorkInfo?.firstOrNull()
    val isCalendarSyncRunning =
        currentWorkInfo?.state == WorkInfo.State.RUNNING || currentWorkInfo?.state == WorkInfo.State.ENQUEUED

    // React to completion to show Toasts
    LaunchedEffect(currentWorkInfo?.state) {
        if (currentWorkInfo?.state == WorkInfo.State.SUCCEEDED) {
            val msg = currentWorkInfo.outputData.getString(CalendarSyncWorker.OUTPUT_MESSAGE)
            if (msg != null) {
                Toast.makeText(context, msg, Toast.LENGTH_LONG).show()
            }
        } else if (currentWorkInfo?.state == WorkInfo.State.FAILED) {
            val msg = currentWorkInfo.outputData.getString(CalendarSyncWorker.OUTPUT_MESSAGE) ?: "Unknown error"
            Toast.makeText(context, msg, Toast.LENGTH_LONG).show()
        }
    }

    // --- OBSERVE MIGRATION WORKER ---
    val migrationWorkInfo by workManager
        .getWorkInfosForUniqueWorkLiveData(CalendarMigrationWorker.UNIQUE_WORK_NAME)
        .observeAsState()

    val currentMigration = migrationWorkInfo?.firstOrNull()

    // Show toast when migration finishes
    LaunchedEffect(currentMigration?.state) {
        if (currentMigration?.state == WorkInfo.State.SUCCEEDED) {
            val msg = currentMigration.outputData.getString(CalendarMigrationWorker.OUTPUT_MESSAGE)
            Toast.makeText(context, msg ?: "Migration complete", Toast.LENGTH_LONG).show()
            // Force refresh UI to show tasks in their new location
            val intent = Intent("com.cfait.REFRESH_UI")
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)
        } else if (currentMigration?.state == WorkInfo.State.FAILED) {
            val msg = currentMigration.outputData.getString(CalendarMigrationWorker.OUTPUT_MESSAGE)
            Toast.makeText(context, msg ?: "Migration failed", Toast.LENGTH_LONG).show()
        }
    }
    // --------------------------------

    val launcher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { }

    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    context,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }

    fun refreshLists() {
        android.util.Log.d("CfaitMain", "refreshLists() called")
        scope.launch {
            try {
                refreshTick = System.currentTimeMillis()
                calendars = api.getCalendars()
                tags = api.getAllTags()
                locations = api.getAllLocations()
                defaultCalHref = api.getConfig().defaultCalendar
                hasUnsynced = api.hasUnsyncedChanges()
                AlarmScheduler.scheduleNextAlarm(context, api)
            } catch (e: Exception) {
                android.util.Log.e("CfaitMain", "Exception in refreshLists", e)
            }
        }
    }

    DisposableEffect(Unit) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                if (intent?.action == "com.cfait.REFRESH_UI") {
                    android.util.Log.d("CfaitMain", "Received REFRESH_UI broadcast")
                    refreshLists()
                }
            }
        }
        val filter = IntentFilter("com.cfait.REFRESH_UI")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(receiver, filter)
        }
        onDispose {
            context.unregisterReceiver(receiver)
        }
    }

    fun fastStart() {
        refreshLists()
        scope.launch {
            isLoading = true
            try {
                api.sync()
                refreshLists()
                val request = OneTimeWorkRequestBuilder<AlarmWorker>().build()
                WorkManager.getInstance(context).enqueueUniqueWork(
                    "cfait_manual_check",
                    ExistingWorkPolicy.KEEP,
                    request
                )
            } catch (e: Exception) {
            }
            isLoading = false
        }
    }

    fun saveTaskInBackground(uid: String, smart: String, desc: String) {
        scope.launch {
            try {
                api.updateTaskSmart(uid, smart)
                api.updateTaskDescription(uid, desc)
                refreshLists()
            } catch (e: Exception) {
                Toast.makeText(context, "Background sync failed: ${e.message}", Toast.LENGTH_LONG).show()
                refreshLists()
            }
        }
    }

    // Updated Action: Use WorkManager
    fun handleDeleteEvents() {
        val workRequest = OneTimeWorkRequestBuilder<CalendarSyncWorker>()
            .setInputData(Data.Builder().putString(CalendarSyncWorker.KEY_MODE, CalendarSyncWorker.MODE_DELETE).build())
            .build()

        workManager.enqueueUniqueWork(
            CalendarSyncWorker.UNIQUE_WORK_NAME,
            ExistingWorkPolicy.REPLACE,
            workRequest
        )
        Toast.makeText(context, "Deleting events in background...", Toast.LENGTH_SHORT).show()
    }

    // Updated Action: Use WorkManager
    fun handleCreateMissingEvents() {
        val workRequest = OneTimeWorkRequestBuilder<CalendarSyncWorker>()
            .setInputData(Data.Builder().putString(CalendarSyncWorker.KEY_MODE, CalendarSyncWorker.MODE_CREATE).build())
            .build()

        workManager.enqueueUniqueWork(
            CalendarSyncWorker.UNIQUE_WORK_NAME,
            ExistingWorkPolicy.REPLACE, // Restart if clicked again
            workRequest
        )
        Toast.makeText(context, "Creating events in background...", Toast.LENGTH_SHORT).show()
    }

    // DEFINE MIGRATION ACTION HANDLER
    fun handleMigration(sourceHref: String, targetHref: String) {
        val workRequest = OneTimeWorkRequestBuilder<CalendarMigrationWorker>()
            .setInputData(
                Data.Builder()
                    .putString(CalendarMigrationWorker.KEY_SOURCE_HREF, sourceHref)
                    .putString(CalendarMigrationWorker.KEY_TARGET_HREF, targetHref)
                    .build()
            )
            .build()

        workManager.enqueueUniqueWork(
            CalendarMigrationWorker.UNIQUE_WORK_NAME,
            ExistingWorkPolicy.KEEP, // Don't run two migrations at once
            workRequest
        )
        Toast.makeText(context, "Migrating tasks in background...", Toast.LENGTH_SHORT).show()
    }

    LaunchedEffect("fastStart") { fastStart() }

    NavHost(navController, startDestination = "home") {
        composable("home") {
            HomeScreen(
                api = api,
                calendars = calendars,
                tags = tags,
                locations = locations,
                defaultCalHref = defaultCalHref,
                isLoading = isLoading,
                hasUnsynced = hasUnsynced,
                autoScrollUid = autoScrollUid,
                refreshTick = refreshTick,
                onGlobalRefresh = { fastStart() },
                onSettings = { navController.navigate("settings") },
                onTaskClick = { uid -> navController.navigate("detail/$uid") },
                onDataChanged = { refreshLists() },
                onMigrateLocal = { sourceHref, targetHref -> handleMigration(sourceHref, targetHref) }
            )
        }
        composable("detail/{uid}") { backStackEntry ->
            val uid = backStackEntry.arguments?.getString("uid")
            if (uid != null) {
                TaskDetailScreen(
                    api = api,
                    uid = uid,
                    calendars = calendars,
                    onBack = {
                        navController.popBackStack()
                        refreshLists()
                    },
                    onSave = { smart, desc ->
                        saveTaskInBackground(uid, smart, desc)
                        autoScrollUid = uid
                        navController.popBackStack()
                        refreshLists()
                    },
                )
            }
        }
        composable("settings") {
            SettingsScreen(
                api = api,
                onBack = {
                    navController.popBackStack()
                    refreshLists()
                },
                onHelp = { navController.navigate("help") },
                isCalendarBusy = isLoading || isCalendarSyncRunning, // Bound to WorkManager status
                onDeleteEvents = { handleDeleteEvents() },
                onCreateEvents = { handleCreateMissingEvents() }
            )
        }
        composable("help") {
            HelpScreen(onBack = { navController.popBackStack() })
        }
    }
}
