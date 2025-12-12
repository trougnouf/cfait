package com.cfait

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileCalendar
import com.cfait.core.MobileTag
import com.cfait.ui.HomeScreen
import com.cfait.ui.SettingsScreen
import com.cfait.ui.TaskDetailScreen
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val api = CfaitMobile(filesDir.absolutePath)
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                CfaitNavHost(api)
            }
        }
    }
}

@Composable
fun CfaitNavHost(api: CfaitMobile) {
    val navController = rememberNavController()
    var calendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var tags by remember { mutableStateOf<List<MobileTag>>(emptyList()) }
    var defaultCalHref by remember { mutableStateOf<String?>(null) }
    
    val scope = rememberCoroutineScope()
    var isLoading by remember { mutableStateOf(false) }
    
    // Yank state needs to persist across navigation (at least partially) or be managed at root
    // For now we keep it in HomeScreen as navigation usually keeps the state in backstack.

    fun fastStart() {
        api.loadFromCache()
        calendars = api.getCalendars()
        scope.launch { tags = api.getAllTags() }
        defaultCalHref = api.getConfig().defaultCalendar
        scope.launch {
            isLoading = true
            try { 
                api.sync()
                calendars = api.getCalendars()
                tags = api.getAllTags()
            } catch (e: Exception) { 
                // Error handling can be added here if needed, or status message passed down
            }
            isLoading = false
        }
    }

    fun refreshLists() {
        scope.launch {
            try {
                calendars = api.getCalendars()
                tags = api.getAllTags()
                defaultCalHref = api.getConfig().defaultCalendar
            } catch (e: Exception) { }
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