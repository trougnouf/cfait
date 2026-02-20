/* File: ./android/app/src/main/java/com/trougnouf/cfait/ui/SettingsScreen.kt
 *
 * Settings screen for the Android client (Compose).
 * This variant moves Trash Retention to the Advanced Settings screen,
 * so the inline trash field and its state have been removed.
 */

package com.trougnouf.cfait.ui

import android.content.Intent
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileCalendar
import kotlinx.coroutines.launch
import java.io.File

private val busyMessages = listOf(
    "Processing stuff", "BRB", "Be right back", "Working on things",
    "Loading", "AFK", "Be right back", "Processing things",
    "Reticulating splines", "Swapping time streams",
    "Defragmenting memories", "Sorting mental baggage", "Getting things done"
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    api: CfaitMobile,
    onBack: () -> Unit,
    onHelp: () -> Unit,
    onAdvanced: () -> Unit,
    isCalendarBusy: Boolean,
    onDeleteEvents: () -> Unit,
    onCreateEvents: () -> Unit,
    currentTheme: String,
    onThemeChange: (String) -> Unit
) {
    var url by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var pass by remember { mutableStateOf("") }
    var insecure by remember { mutableStateOf(false) }
    var hideCompleted by remember { mutableStateOf(false) }
    var sortMonths by remember { mutableStateOf("2") }
    var status by remember { mutableStateOf("") }
    var aliases by remember { mutableStateOf<Map<String, List<String>>>(emptyMap()) }
    var newAliasKey by remember { mutableStateOf("") }
    var newAliasTags by remember { mutableStateOf("") }
    var allCalendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var disabledSet by remember { mutableStateOf<Set<String>>(emptySet()) }
    var urgentDays by remember { mutableStateOf("1") }
    var urgentPrio by remember { mutableStateOf("1") }
    var defaultPriority by remember { mutableStateOf("5") }
    var startGracePeriodDays by remember { mutableStateOf("1") }
    var autoRemind by remember { mutableStateOf(true) }
    var defTime by remember { mutableStateOf("09:00") }
    var autoRefresh by remember { mutableStateOf("30m") }
    var createEventsForTasks by remember { mutableStateOf(false) }
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }

    // NEW STATES
    var maxDoneRoots by remember { mutableStateOf("20") }
    var maxDoneSubtasks by remember { mutableStateOf("5") }

    var debugStatus by remember { mutableStateOf("") }
    var themeExpanded by remember { mutableStateOf(false) }
    val themeOptions = remember {
        val list = mutableListOf(
            "auto" to "Auto-detect",
            "light" to "Light",
            "dark" to "Dark"
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            list.add("dynamic_light" to "Dynamic Light")
            list.add("dynamic_dark" to "Dynamic Dark")
        }
        list
    }

    var initialCreateEventsState by remember { mutableStateOf(false) }
    var isInitialLoad by remember { mutableStateOf(true) }

    var currentBusyMessage by remember { mutableStateOf(busyMessages.first()) }
    LaunchedEffect(isCalendarBusy) {
        if (isCalendarBusy) {
            currentBusyMessage = busyMessages.random()
        }
    }

    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    fun formatDuration(m: UInt): String {
        val min = m.toInt()
        return when {
            min == 0 -> ""
            min % 525600 == 0 -> "${min / 525600}y"
            min % 43200 == 0 -> "${min / 43200}mo"
            min % 10080 == 0 -> "${min / 10080}w"
            min % 1440 == 0 -> "${min / 1440}d"
            min % 60 == 0 -> "${min / 60}h"
            else -> "${min}m"
        }
    }

    fun reload() {
        val cfg = api.getConfig()
        url = cfg.url
        user = cfg.username
        insecure = cfg.allowInsecure
        hideCompleted = cfg.hideCompleted
        sortMonths = cfg.sortCutoffMonths?.toString() ?: ""
        aliases = cfg.tagAliases
        allCalendars = api.getCalendars()
        disabledSet = allCalendars.filter { it.isDisabled }.map { it.href }.toSet()
        urgentDays = cfg.urgentDays.toString()
        urgentPrio = cfg.urgentPrio.toString()
        defaultPriority = cfg.defaultPriority.toString()
        startGracePeriodDays = cfg.startGracePeriodDays.toString()
        autoRemind = cfg.autoReminders
        defTime = cfg.defaultReminderTime
        autoRefresh = formatDuration(cfg.autoRefreshInterval)
        createEventsForTasks = cfg.createEventsForTasks
        deleteEventsOnCompletion = cfg.deleteEventsOnCompletion

        // Load advanced values
        maxDoneRoots = cfg.maxDoneRoots.toString()
        maxDoneSubtasks = cfg.maxDoneSubtasks.toString()

        if (isInitialLoad) {
            initialCreateEventsState = cfg.createEventsForTasks
            isInitialLoad = false
        }
    }

    var importTargetHref by remember { mutableStateOf<String?>(null) }
    val importLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri ->
        uri?.let {
            scope.launch {
                try {
                    val inputStream = context.contentResolver.openInputStream(uri)
                    val icsContent = inputStream?.bufferedReader()?.use { it.readText() }
                    inputStream?.close()

                    if (icsContent != null && importTargetHref != null) {
                        val result = api.importLocalIcs(importTargetHref!!, icsContent)
                        status = result
                        reload()
                    } else {
                        status = "Error: Could not read file"
                    }
                } catch (e: Exception) {
                    status = "Import Error: ${e.message}"
                }
            }
        }
    }

    LaunchedEffect(Unit) { reload() }

    fun saveToDisk() {
        val sortInt = sortMonths.trim().toUIntOrNull()
        val daysInt = urgentDays.trim().toUIntOrNull() ?: 1u
        val prioInt = urgentPrio.trim().toUByteOrNull() ?: 1u
        val defaultPrioInt = defaultPriority.trim().toUByteOrNull() ?: 5u
        val startGraceInt = startGracePeriodDays.trim().toUIntOrNull() ?: 1u

        // Use the current backend config for defaults when UI input is empty
        val cfg = api.getConfig()
        val sShort = cfg.snoozeShort
        val aRefresh = api.parseDurationString(autoRefresh) ?: 30u

        // Trash retention moved to Advanced Settings; use backend-configured value here.
        val trashInt = cfg.trashRetention

        // parse advanced numeric inputs; fall back to backend config if UI empty
        val maxRootsInt = maxDoneRoots.trim().toUIntOrNull() ?: cfg.maxDoneRoots.toUInt()
        val maxSubtasksInt = maxDoneSubtasks.trim().toUIntOrNull() ?: cfg.maxDoneSubtasks.toUInt()

        // Ensure arguments match Rust signature exactly:
        // url, user, pass, insecure, hide_completed, disabled_cals, sort, days, prio,
        // default_prio, grace, auto_reminders, default_time, snooze_short, create_events,
        // delete_events, auto_refresh, trash_retention, max_done_roots, max_done_subtasks
        api.saveConfig(
            url, user, pass, insecure, hideCompleted,
            disabledSet.toList(), sortInt,
            daysInt, prioInt, defaultPrioInt, startGraceInt,
            autoRemind, defTime, sShort,
            createEventsForTasks, deleteEventsOnCompletion,
            aRefresh,
            trashInt,
            maxRootsInt, maxSubtasksInt
        )
    }

    fun saveAndConnect() {
        scope.launch {
            status = "Connecting..."
            try {
                saveToDisk()
                status = api.connect(url, user, pass, insecure)
                reload()
            } catch (e: Exception) {
                status = "Connection failed: ${e.message}"
            }
        }
    }

    fun saveAndExit() {
        saveToDisk()
        onBack()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize()
        ) {
            // Connection Section
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                label = { Text("CalDAV Server URL") },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = user,
                onValueChange = { user = it },
                label = { Text("Username") },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = pass,
                onValueChange = { pass = it },
                label = { Text("Password") },
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = insecure, onCheckedChange = { insecure = it })
                Spacer(Modifier.width(8.dp))
                Text("Allow insecure SSL (e.g. self-signed)")
            }
            Spacer(Modifier.height(12.dp))

            // Preferences / Behavior
            Text("Preferences", fontWeight = FontWeight.Bold, fontSize = 18.sp)
            Spacer(Modifier.height(8.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Hide completed tasks")
                Spacer(Modifier.width(8.dp))
                Switch(checked = hideCompleted, onCheckedChange = { hideCompleted = it })
            }
            Spacer(Modifier.height(8.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Auto-remind on dates")
                Spacer(Modifier.width(8.dp))
                Switch(checked = autoRemind, onCheckedChange = { autoRemind = it })
            }
            Spacer(Modifier.height(8.dp))

            OutlinedTextField(
                value = defTime,
                onValueChange = { defTime = it },
                label = { Text("Default reminder time (HH:MM)") },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(12.dp))

            // Calendar Integration
            Text("Calendar integration", fontWeight = FontWeight.Bold, fontSize = 18.sp)
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(
                    checked = createEventsForTasks,
                    onCheckedChange = { createEventsForTasks = it }
                )
                Spacer(Modifier.width(8.dp))
                Text("Create calendar events (VEVENT) for tasks with dates")
            }
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(
                    checked = deleteEventsOnCompletion,
                    onCheckedChange = { deleteEventsOnCompletion = it }
                )
                Spacer(Modifier.width(8.dp))
                Text("Delete calendar events when tasks are completed")
            }
            Spacer(Modifier.height(8.dp))

            // Advanced navigation
            Button(onClick = onAdvanced, modifier = Modifier.fillMaxWidth()) {
                Text("Advanced Settings")
            }
            Spacer(Modifier.height(12.dp))

            // Local collections import/export
            Text("Local collections", fontWeight = FontWeight.Bold, fontSize = 18.sp)
            Spacer(Modifier.height(8.dp))
            LazyColumn {
                items(allCalendars) { cal ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(cal.name, modifier = Modifier.weight(1f))
                        Button(onClick = {
                            importTargetHref = cal.href
                            importLauncher.launch("*/*")
                        }) {
                            Text("Import ICS")
                        }
                    }
                }
            }

            Spacer(Modifier.height(16.dp))

            // Bottom actions
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Button(onClick = {
                    saveAndConnect()
                }) {
                    Text("Save & Connect")
                }
                Button(onClick = {
                    saveToDisk()
                    status = "Saved"
                }) {
                    Text("Save")
                }
            }

            if (status.isNotEmpty()) {
                Spacer(Modifier.height(8.dp))
                Text(status)
            }
        }
    }
}
