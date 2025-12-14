// File: ./android/app/src/main/java/com/cfait/MainActivity.kt
package com.cfait

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.compose.runtime.*
import com.cfait.core.MobileCalendar
import com.cfait.core.MobileTag
import com.cfait.ui.HomeScreen
import com.cfait.ui.SettingsScreen
import com.cfait.ui.TaskDetailScreen
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Use the singleton instance from Application to persist across rotations
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
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    var hasUnsynced by remember { mutableStateOf(false) }
    
    val scope = rememberCoroutineScope()
    var isLoading by remember { mutableStateOf(false) }
    
    fun refreshLists() {
        scope.launch {
            try {
                // Load form memory/cache immediately
                calendars = api.getCalendars()
                tags = api.getAllTags()
                defaultCalHref = api.getConfig().defaultCalendar
                hasUnsynced = api.hasUnsyncedChanges()
            } catch (e: Exception) { }
        }
    }

    fun fastStart() {
        // Initial UI load from memory
        refreshLists()
        
        // Trigger network sync
        scope.launch {
            isLoading = true
            try { 
                api.sync()
                refreshLists()
            } catch (e: Exception) { 
                // Error handling can be passed down via state if needed
            }
            isLoading = false
        }
    }

    LaunchedEffect(Unit) { fastStart() }

    NavHost(navController, startDestination = "home") {
        composable("home") {
            HomeScreen(
                api = api,
                calendars = calendars,
                tags = tags,
                defaultCalHref = defaultCalHref,
                isLoading = isLoading,
                hasUnsynced = hasUnsynced,
                onGlobalRefresh = { fastStart() },
                onSettings = { navController.navigate("settings") },
                onTaskClick = { uid -> navController.navigate("detail/$uid") },
                onDataChanged = { refreshLists() }
            )
        }
        composable("detail/{uid}") { backStackEntry ->
            val uid = backStackEntry.arguments?.getString("uid")
            if (uid != null) {
                TaskDetailScreen(
                    api = api,
                    uid = uid,
                    calendars = calendars,
                    onBack = { navController.popBackStack(); refreshLists() }
                )
            }
        }
        composable("settings") {
            SettingsScreen(api = api, onBack = { navController.popBackStack(); refreshLists() })
        }
    }
}