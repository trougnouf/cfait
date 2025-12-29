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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
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
    var calendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var tags by remember { mutableStateOf<List<MobileTag>>(emptyList()) }
    var locations by remember { mutableStateOf<List<MobileLocation>>(emptyList()) }
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    var hasUnsynced by remember { mutableStateOf(false) }
    var autoScrollUid by remember { mutableStateOf<String?>(null) }

    // FIX: Add a version counter to force UI updates even if data lists look identical
    var refreshTick by remember { mutableLongStateOf(System.currentTimeMillis()) }

    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var isLoading by remember { mutableStateOf(false) }

    // Hoisted state for calendar event deletion (persists across navigation)
    var isDeletingEvents by rememberSaveable { mutableStateOf(false) }

    // Hoisted state for calendar event backfill (persists across navigation)
    var isBackfilling by rememberSaveable { mutableStateOf(false) }

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
                // FIX: Update tick first to ensure downstream effects trigger
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

    fun handleDeleteEvents() {
        if (isDeletingEvents) return
        isDeletingEvents = true
        scope.launch {
            try {
                val count = api.deleteAllCalendarEvents()
                Toast.makeText(
                    context,
                    "Deleted $count calendar event${if (count == 1u) "" else "s"}",
                    Toast.LENGTH_LONG
                ).show()
            } catch (e: Exception) {
                Toast.makeText(context, "Error: ${e.message}", Toast.LENGTH_LONG).show()
            } finally {
                isDeletingEvents = false
            }
        }
    }

    fun handleCreateMissingEvents() {
        if (isBackfilling) return
        isBackfilling = true
        scope.launch {
            try {
                Toast.makeText(context, "Creating calendar events in background...", Toast.LENGTH_SHORT).show()

                val count = api.createMissingCalendarEvents()

                Toast.makeText(
                    context,
                    "Created $count missing calendar event${if (count == 1u) "" else "s"}",
                    Toast.LENGTH_LONG
                ).show()
            } catch (e: Exception) {
                Toast.makeText(context, "Backfill error: ${e.message}", Toast.LENGTH_LONG).show()
            } finally {
                isBackfilling = false
            }
        }
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
                refreshTick = refreshTick, // FIX: Pass the tick
                onGlobalRefresh = { fastStart() },
                onSettings = { navController.navigate("settings") },
                onTaskClick = { uid -> navController.navigate("detail/$uid") },
                onDataChanged = { refreshLists() },
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
                isCalendarBusy = isLoading || isDeletingEvents || isBackfilling,
                onDeleteEvents = { handleDeleteEvents() },
                onCreateEvents = { handleCreateMissingEvents() }
            )

        }
        composable("help") {
            HelpScreen(onBack = { navController.popBackStack() })
        }
    }
}
