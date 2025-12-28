package com.cfait

import android.Manifest
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
    var locations by remember { mutableStateOf<List<MobileLocation>>(emptyList()) } // <--- Added
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    var hasUnsynced by remember { mutableStateOf(false) }

    // State to trigger scrolling in HomeScreen
    var autoScrollUid by remember { mutableStateOf<String?>(null) }

    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var isLoading by remember { mutableStateOf(false) }

    // --- Permission Request Logic ---
    val launcher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { isGranted: Boolean ->
        if (isGranted) {
            // Permission granted
        } else {
            // Permission denied
        }
    }

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
    // -------------------------------

    fun refreshLists() {
        android.util.Log.d("CfaitMain", "refreshLists() called - will schedule alarms")
        scope.launch {
            try {
                calendars = api.getCalendars()
                tags = api.getAllTags()
                locations = api.getAllLocations() // <--- Fetch locations
                defaultCalHref = api.getConfig().defaultCalendar
                hasUnsynced = api.hasUnsyncedChanges()

                // [FIX ADDED] Schedule the next alarm whenever the data changes
                // This handles Add, Edit, Delete, Toggle, and Sync scenarios.
                android.util.Log.d("CfaitMain", "About to call scheduleNextAlarm from refreshLists")
                AlarmScheduler.scheduleNextAlarm(context, api)

            } catch (e: Exception) {
                android.util.Log.e("CfaitMain", "Exception in refreshLists", e)
            }
        }
    }

    fun fastStart() {
        refreshLists()
        scope.launch {
            isLoading = true
            try {
                api.sync()
                refreshLists()

                // 1. Schedule the next *future* alarm
                AlarmScheduler.scheduleNextAlarm(context, api)

                // 2. Immediately check for alarms that should be firing *now*
                // (or were missed in the last 2 hours).
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

    // This runs in the NavHost scope, so it survives screen transitions
    fun saveTaskInBackground(
        uid: String,
        smart: String,
        desc: String,
    ) {
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

    LaunchedEffect("fastStart") { fastStart() }

    NavHost(navController, startDestination = "home") {
        composable("home") {
            HomeScreen(
                api = api,
                calendars = calendars,
                tags = tags,
                locations = locations, // <--- Pass locations
                defaultCalHref = defaultCalHref,
                isLoading = isLoading,
                hasUnsynced = hasUnsynced,
                autoScrollUid = autoScrollUid,
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
                        // Trigger scroll on return
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
            )
        }
        composable("help") {
            HelpScreen(onBack = { navController.popBackStack() })
        }
    }
}
