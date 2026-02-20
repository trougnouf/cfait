// File: ./android/app/src/main/java/com/trougnouf/cfait/MainActivity.kt
package com.trougnouf.cfait

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
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
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.Constraints
import androidx.work.NetworkType
import java.util.concurrent.TimeUnit
import com.trougnouf.cfait.workers.PeriodicSyncWorker
import com.trougnouf.cfait.core.MobileCalendar
import com.trougnouf.cfait.core.MobileLocation
import com.trougnouf.cfait.core.MobileTag
import com.trougnouf.cfait.ui.HelpScreen
import com.trougnouf.cfait.ui.HomeScreen
import com.trougnouf.cfait.ui.IcsImportScreen
import com.trougnouf.cfait.ui.SettingsScreen
import com.trougnouf.cfait.ui.AdvancedSettingsScreen
import com.trougnouf.cfait.ui.TaskDetailScreen
import com.trougnouf.cfait.util.AlarmScheduler
import com.trougnouf.cfait.workers.AlarmWorker
import com.trougnouf.cfait.workers.CalendarMigrationWorker
import com.trougnouf.cfait.workers.CalendarSyncWorker
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val app = application as CfaitApplication
        val api = app.api

        // Retrieve saved theme preference (defaulting to "auto")
        val sharedPrefs = getSharedPreferences("cfait_ui_prefs", Context.MODE_PRIVATE)
        val savedTheme = sharedPrefs.getString("app_theme", "auto") ?: "auto"

        setContent {
            // Lift theme state to root so SettingsScreen can update it
            var currentTheme by remember { mutableStateOf(savedTheme) }

            // Determine the color scheme based on preference and system state
            val context = LocalContext.current
            val systemInDark = isSystemInDarkTheme()
            val dynamicAvailable = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

            val colorScheme: ColorScheme = remember(currentTheme, systemInDark) {
                when (currentTheme) {
                    "light" -> lightColorScheme()
                    "dark" -> darkColorScheme()
                    "dynamic_light" -> if (dynamicAvailable) dynamicLightColorScheme(context) else lightColorScheme()
                    "dynamic_dark" -> if (dynamicAvailable) dynamicDarkColorScheme(context) else darkColorScheme()
                    // Auto: Prefer dynamic if available, otherwise standard
                    else -> {
                        if (dynamicAvailable) {
                            if (systemInDark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
                        } else {
                            if (systemInDark) darkColorScheme() else lightColorScheme()
                        }
                    }
                }
            }

            MaterialTheme(colorScheme = colorScheme) {
                CfaitNavHost(
                    api = api,
                    intent = intent,
                    currentTheme = currentTheme,
                    onThemeChange = { newTheme ->
                        currentTheme = newTheme
                        sharedPrefs.edit().putString("app_theme", newTheme).apply()
                    }
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        recreate()
    }
}

@Composable
fun CfaitNavHost(
    api: com.trougnouf.cfait.core.CfaitMobile,
    intent: Intent? = null,
    currentTheme: String,
    onThemeChange: (String) -> Unit
) {
    val navController = rememberNavController()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    // Data State
    var calendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    var hasUnsynced by remember { mutableStateOf(false) }
    // Add state for default priority
    var defaultPriority by remember { mutableIntStateOf(5) }
    var autoScrollUid by remember { mutableStateOf<String?>(null) }
    var refreshTick by remember { mutableLongStateOf(System.currentTimeMillis()) }
    var isLoading by remember { mutableStateOf(false) }

    // Lift state for advanced settings so it persists across navigation
    var maxDoneRoots by remember { mutableStateOf("20") }
    var maxDoneSubtasks by remember { mutableStateOf("5") }

    // Init from config on load
    LaunchedEffect(Unit) {
        try {
            val cfg = api.getConfig()
            maxDoneRoots = cfg.maxDoneRoots.toString()
            maxDoneSubtasks = cfg.maxDoneSubtasks.toString()
        } catch (e: Exception) {
            if (e is CancellationException) throw e
            // ignore init errors here
        }
    }

    // ICS Import State
    var icsContentToImport by remember { mutableStateOf<String?>(null) }

    // --- WORK MANAGER OBSERVATION ---
    val workManager = WorkManager.getInstance(context)

    // Observe the specific unique work we queue for calendar sync
    val calendarWorkInfo by workManager
        .getWorkInfosForUniqueWorkLiveData(CalendarSyncWorker.UNIQUE_WORK_NAME)
        .observeAsState()

    // Schedule a periodic background sync worker to keep remote changes and alarms up-to-date.
    // Uses a 30-minute interval; WorkManager will coalesce as appropriate.
    val periodicSyncRequest = PeriodicWorkRequestBuilder<PeriodicSyncWorker>(30, TimeUnit.MINUTES)
        .setConstraints(
            Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()
        )
        .build()

    WorkManager.getInstance(context).enqueueUniquePeriodicWork(
        "cfait_periodic_sync",
        ExistingPeriodicWorkPolicy.KEEP,
        periodicSyncRequest
    )

    val currentWorkInfo = calendarWorkInfo?.firstOrNull()
    val isCalendarSyncRunning =
        currentWorkInfo?.state == WorkInfo.State.RUNNING || currentWorkInfo?.state == WorkInfo.State.ENQUEUED

    // React to completion to show Toasts
    // NOTE: Suppress toasts on FAILED for periodic/background sync to avoid alarming the user.
    LaunchedEffect(currentWorkInfo?.state) {
        try {
            if (currentWorkInfo?.state == WorkInfo.State.SUCCEEDED) {
                val msg = currentWorkInfo.outputData.getString(CalendarSyncWorker.OUTPUT_MESSAGE)
                if (msg != null) {
                    Toast.makeText(context, msg, Toast.LENGTH_LONG).show()
                }
            } else if (currentWorkInfo?.state == WorkInfo.State.FAILED) {
                // Do not show a toast for periodic/background sync failures to avoid alarming the user.
                // Log the failure for diagnostics instead.
                val msg = currentWorkInfo.outputData.getString(CalendarSyncWorker.OUTPUT_MESSAGE) ?: "Unknown error"
                android.util.Log.w("CfaitMain", "Periodic calendar sync failed: $msg")
            }
        } catch (e: Exception) {
            if (e is CancellationException) throw e
            android.util.Log.e("CfaitMain", "Error handling WorkManager state change", e)
        }
    }

    // --- OBSERVE MIGRATION WORKER ---
    val migrationWorkInfo by workManager
        .getWorkInfosForUniqueWorkLiveData(CalendarMigrationWorker.UNIQUE_WORK_NAME)
        .observeAsState()

    val currentMigration = migrationWorkInfo?.firstOrNull()

    LaunchedEffect(currentMigration?.state) {
        try {
            if (currentMigration?.state == WorkInfo.State.SUCCEEDED) {
                val msg = currentMigration.outputData.getString(CalendarMigrationWorker.OUTPUT_MESSAGE)
                Toast.makeText(context, msg ?: "Migration complete", Toast.LENGTH_LONG).show()

                // Force refresh UI
                val intent = Intent("com.trougnouf.cfait.REFRESH_UI")
                intent.setPackage(context.packageName)
                context.sendBroadcast(intent)

                // Prune the work so the Toast doesn't appear on next launch
                workManager.pruneWork()
            } else if (currentMigration?.state == WorkInfo.State.FAILED) {
                val msg = currentMigration.outputData.getString(CalendarMigrationWorker.OUTPUT_MESSAGE)
                Toast.makeText(context, msg ?: "Migration failed", Toast.LENGTH_LONG).show()

                // Prune failed work too so user can retry immediately without UI glitch
                workManager.pruneWork()
            }
        } catch (e: Exception) {
            if (e is CancellationException) throw e
            android.util.Log.e("CfaitMain", "Error observing migration worker", e)
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
                val config = api.getConfig()
                calendars = api.getCalendars()
                defaultCalHref = config.defaultCalendar
                defaultPriority = config.defaultPriority.toInt() // Fetch config value
                hasUnsynced = api.hasUnsyncedChanges()
                AlarmScheduler.scheduleNextAlarm(context, api)
            } catch (e: Exception) {
                if (e is CancellationException) throw e
                android.util.Log.e("CfaitMain", "Exception in refreshLists", e)
            }
        }
    }

    DisposableEffect(Unit) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                if (intent?.action == "com.trougnouf.cfait.REFRESH_UI") {
                    android.util.Log.d("CfaitMain", "Received REFRESH_UI broadcast")
                    refreshLists()
                }
            }
        }
        val filter = IntentFilter("com.trougnouf.cfait.REFRESH_UI")
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
                if (e is CancellationException) throw e
                android.util.Log.e("CfaitMain", "fastStart failed", e)
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
                if (e is CancellationException) throw e // IGNORE CANCELLATION (don't show to user)
                Toast.makeText(context, "Background sync failed: ${e.message}", Toast.LENGTH_LONG).show()
                refreshLists()
            }
        }
    }

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

    fun handleCreateMissingEvents() {
        val workRequest = OneTimeWorkRequestBuilder<CalendarSyncWorker>()
            .setInputData(Data.Builder().putString(CalendarSyncWorker.KEY_MODE, CalendarSyncWorker.MODE_CREATE).build())
            .build()

        workManager.enqueueUniqueWork(
            CalendarSyncWorker.UNIQUE_WORK_NAME,
            ExistingWorkPolicy.REPLACE,
            workRequest
        )
        Toast.makeText(context, "Creating events in background...", Toast.LENGTH_SHORT).show()
    }

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
            ExistingWorkPolicy.KEEP,
            workRequest
        )
        Toast.makeText(context, "Migrating tasks in background...", Toast.LENGTH_SHORT).show()
    }

    LaunchedEffect("fastStart") { fastStart() }

    LaunchedEffect(intent) {
        intent?.let {
            if (it.action == Intent.ACTION_VIEW) {
                val uri: Uri? = it.data
                uri?.let { fileUri ->
                    try {
                        val inputStream = context.contentResolver.openInputStream(fileUri)
                        val icsContent = inputStream?.bufferedReader()?.use { reader -> reader.readText() }
                        inputStream?.close()

                        if (icsContent != null) {
                            icsContentToImport = icsContent
                            navController.navigate("ics_import")
                        } else {
                            Toast.makeText(context, "Failed to read ICS file", Toast.LENGTH_LONG).show()
                        }
                    } catch (e: Exception) {
                        if (e is CancellationException) throw e
                        Toast.makeText(context, "Error opening file: ${e.message}", Toast.LENGTH_LONG).show()
                    }
                }
            }
        }
    }

    NavHost(navController, startDestination = "home") {
        composable("settings/advanced") {
            // Load fresh on entry
            var localRoots by remember { mutableStateOf("20") }
            var localSubs by remember { mutableStateOf("5") }
            var localTrash by remember { mutableStateOf("14") }

            LaunchedEffect(Unit) {
                try {
                    val cfg = api.getConfig()
                    localRoots = cfg.maxDoneRoots.toString()
                    localSubs = cfg.maxDoneSubtasks.toString()
                    localTrash = cfg.trashRetention.toString()
                } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    // ignore
                }
            }

            AdvancedSettingsScreen(
                api = api,
                maxDoneRoots = localRoots,
                maxDoneSubtasks = localSubs,
                trashRetention = localTrash,
                onMaxDoneRootsChange = { localRoots = it },
                onMaxDoneSubtasksChange = { localSubs = it },
                onTrashRetentionChange = { localTrash = it },
                onBack = {
                    // Save on exit
                    try {
                        val cfg = api.getConfig()
                        val r = localRoots.toUIntOrNull() ?: 20u
                        val s = localSubs.toUIntOrNull() ?: 5u
                        val t = localTrash.toUIntOrNull() ?: 14u
                        api.saveConfig(
                            cfg.url, cfg.username, "", cfg.allowInsecure, cfg.hideCompleted,
                            cfg.disabledCalendars, cfg.sortCutoffMonths, cfg.urgentDays, cfg.urgentPrio,
                            cfg.defaultPriority, cfg.startGracePeriodDays, cfg.autoReminders,
                            cfg.defaultReminderTime, cfg.snoozeShort, cfg.createEventsForTasks,
                            cfg.deleteEventsOnCompletion, cfg.autoRefreshInterval,
                            t, r, s // New values (added trash retention)
                        )
                    } catch (e: Exception) {
                        if (e is CancellationException) throw e
                        // swallow save error
                    }
                    navController.popBackStack()
                }
            )
        }
        composable("home") {
            HomeScreen(
                api = api,
                calendars = calendars,
                defaultCalHref = defaultCalHref,
                defaultPriority = defaultPriority, // Pass it here
                isLoading = isLoading,
                hasUnsynced = hasUnsynced,
                autoScrollUid = autoScrollUid,
                refreshTick = refreshTick,
                onGlobalRefresh = { fastStart() },
                onSettings = { navController.navigate("settings") },
                onTaskClick = { uid -> navController.navigate("detail/$uid") },
                onDataChanged = { refreshLists() },
                onMigrateLocal = { sourceHref, targetHref -> handleMigration(sourceHref, targetHref) },
                onAutoScrollComplete = { autoScrollUid = null }
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
                    onNavigate = { targetUid ->
                        autoScrollUid = targetUid
                        navController.popBackStack("home", inclusive = false)
                    }
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
                onAdvanced = { navController.navigate("settings/advanced") },
                isCalendarBusy = isLoading || isCalendarSyncRunning,
                onDeleteEvents = { handleDeleteEvents() },
                onCreateEvents = { handleCreateMissingEvents() },
                currentTheme = currentTheme,
                onThemeChange = onThemeChange
            )
        }

        composable("settings/advanced") {
            // Load fresh on entry
            var localRoots by remember { mutableStateOf("20") }
            var localSubs by remember { mutableStateOf("5") }
            var localTrash by remember { mutableStateOf("14") }

            LaunchedEffect(Unit) {
                try {
                    val cfg = api.getConfig()
                    localRoots = cfg.maxDoneRoots.toString()
                    localSubs = cfg.maxDoneSubtasks.toString()
                    localTrash = cfg.trashRetention.toString()
                } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    // ignore
                }
            }

            AdvancedSettingsScreen(
                api = api,
                maxDoneRoots = localRoots,
                maxDoneSubtasks = localSubs,
                trashRetention = localTrash,
                onMaxDoneRootsChange = { localRoots = it },
                onMaxDoneSubtasksChange = { localSubs = it },
                onTrashRetentionChange = { localTrash = it },
                onBack = {
                    // Save on exit
                    try {
                        val cfg = api.getConfig()
                        val r = localRoots.toUIntOrNull() ?: 20u
                        val s = localSubs.toUIntOrNull() ?: 5u
                        val t = localTrash.toUIntOrNull() ?: 14u
                        api.saveConfig(
                            cfg.url, cfg.username, "", cfg.allowInsecure, cfg.hideCompleted,
                            cfg.disabledCalendars, cfg.sortCutoffMonths, cfg.urgentDays, cfg.urgentPrio,
                            cfg.defaultPriority, cfg.startGracePeriodDays, cfg.autoReminders,
                            cfg.defaultReminderTime, cfg.snoozeShort, cfg.createEventsForTasks,
                            cfg.deleteEventsOnCompletion, cfg.autoRefreshInterval,
                            t, r, s // New values (added trash retention)
                        )
                    } catch (e: Exception) {
                        if (e is CancellationException) throw e
                        // swallow save error
                    }
                    navController.popBackStack()
                }
            )
        }
        composable("help") {
            HelpScreen(onBack = { navController.popBackStack() })
        }
        composable("ics_import") {
            val content = icsContentToImport
            if (content != null) {
                IcsImportScreen(
                    api = api,
                    icsContent = content,
                    calendars = calendars,
                    onImportComplete = { calendarHref ->
                        scope.launch {
                            try {
                                val result = api.importLocalIcs(calendarHref, content)
                                Toast.makeText(context, result, Toast.LENGTH_LONG).show()
                                icsContentToImport = null
                                refreshLists()
                                navController.navigate("home") {
                                    popUpTo("home") { inclusive = false }
                                }
                            } catch (e: Exception) {
                                if (e is CancellationException) throw e
                                Toast.makeText(context, "Import failed: ${e.message}", Toast.LENGTH_LONG).show()
                            }
                        }
                    },
                    onCancel = {
                        icsContentToImport = null
                        navController.navigate("home") {
                            popUpTo("home") { inclusive = false }
                        }
                    }
                )
            }
        }
    }
}
