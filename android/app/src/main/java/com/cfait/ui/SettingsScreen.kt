// File: android/app/src/main/java/com/cfait/ui/SettingsScreen.kt
package com.cfait.ui

import android.content.Intent
import androidx.activity.compose.BackHandler
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileCalendar
import kotlinx.coroutines.launch
import java.io.File

private val busyMessages = listOf(
    "Processing stuff", "BRB", "Be right back", "Working on things",
    "Loading", "AFK", "Busy", "Processing things",
    "Reticulating splines", "Swapping time streams",
    "Defragmenting memories", "Sorting mental baggage", "Getting things done"
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    api: CfaitMobile,
    onBack: () -> Unit,
    onHelp: () -> Unit,
    isCalendarBusy: Boolean,
    onDeleteEvents: () -> Unit,
    onCreateEvents: () -> Unit,
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
    var autoRemind by remember { mutableStateOf(true) }
    var defTime by remember { mutableStateOf("09:00") }
    var snoozeShort by remember { mutableStateOf("1h") }
    var snoozeLong by remember { mutableStateOf("1d") }
    var createEventsForTasks by remember { mutableStateOf(false) }
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }

    // Track initial state to detect toggle transitions
    var initialCreateEventsState by remember { mutableStateOf(false) }
    var isInitialLoad by remember { mutableStateOf(true) }

    // Random busy message that only changes when busy state begins
    var currentBusyMessage by remember { mutableStateOf(busyMessages.first()) }
    LaunchedEffect(isCalendarBusy) {
        if (isCalendarBusy) {
            currentBusyMessage = busyMessages.random()
        }
    }

    val scope = rememberCoroutineScope()

    // Helper to format minutes for display on load
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
        autoRemind = cfg.autoReminders
        defTime = cfg.defaultReminderTime
        snoozeShort = formatDuration(cfg.snoozeShort)
        snoozeLong = formatDuration(cfg.snoozeLong)
        createEventsForTasks = cfg.createEventsForTasks
        deleteEventsOnCompletion = cfg.deleteEventsOnCompletion

        // Capture initial state on first load
        if (isInitialLoad) {
            initialCreateEventsState = cfg.createEventsForTasks
            isInitialLoad = false
        }
    }

    LaunchedEffect(Unit) { reload() }

    fun saveToDisk() {
        val sortInt = sortMonths.trim().toUIntOrNull()
        val daysInt = urgentDays.trim().toUIntOrNull() ?: 1u
        val prioInt = urgentPrio.trim().toUByteOrNull() ?: 1u

        // Use api.parseDurationString instead of toUIntOrNull
        val sShort = api.parseDurationString(snoozeShort) ?: 60u
        val sLong = api.parseDurationString(snoozeLong) ?: 1440u

        api.saveConfig(
            url, user, pass, insecure, hideCompleted,
            disabledSet.toList(), sortInt,
            daysInt, prioInt,
            autoRemind, defTime, sShort, sLong,
            createEventsForTasks, deleteEventsOnCompletion
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
                status = "Error: ${e.message}"
            }
        }
    }

    fun handleBack() {
        scope.launch {
            saveToDisk()

            // If user enabled calendar events (transition from OFF to ON), trigger backfill
            if (!initialCreateEventsState && createEventsForTasks) {
                onCreateEvents()
            }

            onBack()
        }
    }

    // Intercept system back gesture/button to save settings and trigger backfill
    BackHandler {
        handleBack()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = { IconButton(onClick = { handleBack() }) { NfIcon(NfIcons.BACK, 20.sp) } },
                actions = { IconButton(onClick = onHelp) { NfIcon(NfIcons.HELP, 24.sp) } },
            )
        },
    ) { p ->
        LazyColumn(modifier = Modifier.padding(p).padding(16.dp)) {
            item {
                Text(
                    "Server Connection",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("CalDAV URL") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = user,
                    onValueChange = { user = it },
                    label = { Text("Username") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = pass,
                    onValueChange = {
                        pass = it
                    },
                    label = { Text("Password") },
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth()
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = insecure, onCheckedChange = { insecure = it })
                    Text("Allow insecure SSL")
                }
                Button(
                    onClick = { saveAndConnect() },
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp)
                ) { Text("Save & Connect") }
                if (status.isNotEmpty()) {
                    Text(
                        status,
                        color = if (status.startsWith("Error")) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
            }

            item { Text("Manage calendars", fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 8.dp)) }
            items(allCalendars) { cal ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = !disabledSet.contains(cal.href), onCheckedChange = { enabled ->
                        val newSet = disabledSet.toMutableSet()
                        if (enabled) newSet.remove(cal.href) else newSet.add(cal.href)
                        disabledSet = newSet
                        saveToDisk()
                    })
                    Text(cal.name)
                }
            }

            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    "Preferences",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = hideCompleted, onCheckedChange = {
                        hideCompleted = it
                        saveToDisk()
                    })
                    Text("Hide completed and canceled tasks")
                }

                // Urgency Settings
                Text("Urgency sorting rules:", fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 16.dp))

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp)) {
                    Text("Due within (days):", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = urgentDays,
                        onValueChange = { urgentDays = it },
                        modifier = Modifier.width(80.dp),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        singleLine = true
                    )
                }
                Text(
                    "Tasks due in this range show at top.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray
                )

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Top Priority Threshold (!):", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = urgentPrio,
                        onValueChange = { urgentPrio = it },
                        modifier = Modifier.width(80.dp),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        singleLine = true
                    )
                }
                Text(
                    "Priorities <= this value show at top.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray
                )

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Sorting priority cutoff (months):", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = sortMonths,
                        onValueChange = {
                            sortMonths = it
                        },
                        modifier =
                            Modifier.width(
                                80.dp,
                            ),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        singleLine = true,
                    )
                }
                Text(
                    "Tasks due within this range are shown before undated tasks.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray
                )

                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    "Notifications & Reminders",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )

                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = autoRemind, onCheckedChange = { autoRemind = it })
                    Text("Auto-remind on Due/Start")
                }
                Text(
                    "Only if no specific alarms are set.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 12.dp)
                )

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Default time (HH:MM):", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = defTime,
                        onValueChange = { defTime = it },
                        modifier = Modifier.width(100.dp),
                        singleLine = true
                    )
                }

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Snooze Presets:", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = snoozeShort,
                        onValueChange = { snoozeShort = it },
                        modifier = Modifier.width(70.dp),
                        singleLine = true,
                        placeholder = { Text("1h") }
                    )
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = snoozeLong,
                        onValueChange = { snoozeLong = it },
                        modifier = Modifier.width(70.dp),
                        singleLine = true,
                        placeholder = { Text("1d") }
                    )
                }

                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    "Calendar Integration",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )

                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(
                        checked = createEventsForTasks,
                        onCheckedChange = { createEventsForTasks = it },
                        enabled = !isCalendarBusy
                    )
                    Text("Create calendar events for tasks with dates")
                }
                Text(
                    "Events will be retroactively created. Use +cal/-cal per task to override.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 12.dp)
                )

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Checkbox(
                        checked = deleteEventsOnCompletion,
                        onCheckedChange = { deleteEventsOnCompletion = it },
                        enabled = !isCalendarBusy
                    )
                    Text("Delete events when tasks are completed")
                }
                Text(
                    "Regardless, events are always deleted when tasks are deleted.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 12.dp)
                )

                Button(
                    onClick = onDeleteEvents,
                    modifier = Modifier.padding(top = 8.dp),
                    enabled = !isCalendarBusy
                ) {
                    if (isCalendarBusy) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(16.dp),
                                color = MaterialTheme.colorScheme.onPrimary,
                                strokeWidth = 2.dp
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text("$currentBusyMessage...")
                        }
                    } else {
                        Text("Delete all calendar events")
                    }
                }

                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    "Local Calendars",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )

                // Filter local calendars from the state we already have
                val localCals = allCalendars.filter { it.isLocal }

                // Capture context outside the lambda
                val context = LocalContext.current

                localCals.forEach { cal ->
                    LocalCalendarEditor(
                        cal = cal,
                        onUpdate = { name, color ->
                            scope.launch {
                                try {
                                    api.updateLocalCalendar(cal.href, name, color)
                                    reload()
                                } catch (e: Exception) {
                                    status = "Error: ${e.message}"
                                }
                            }
                        },
                        onDelete = {
                            scope.launch {
                                try {
                                    api.deleteLocalCalendar(cal.href)
                                    reload()
                                } catch (e: Exception) {
                                    status = "Error: ${e.message}"
                                }
                            }
                        },
                        onExport = {
                            try {
                                val icsContent = api.exportLocalIcs(cal.href)
                                val calId = cal.href.removePrefix("local://")
                                val file = File(context.cacheDir, "cfait_${calId}.ics")
                                file.writeText(icsContent)
                                val uri = FileProvider.getUriForFile(
                                    context,
                                    "${context.packageName}.fileprovider",
                                    file
                                )
                                val intent = Intent(Intent.ACTION_SEND).apply {
                                    type = "text/calendar"
                                    putExtra(Intent.EXTRA_STREAM, uri)
                                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                }
                                val shareIntent = Intent.createChooser(intent, "Export ${cal.name}")
                                context.startActivity(shareIntent)
                            } catch (e: Exception) {
                                status = "Export Error: ${e.message}"
                            }
                        }
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                }

                Button(
                    onClick = {
                        scope.launch {
                            try {
                                api.createLocalCalendar("New Calendar", null)
                                reload()
                            } catch (e: Exception) {
                                status = "Error: ${e.message}"
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                ) {
                    NfIcon(NfIcons.ADD, 16.sp)
                    Spacer(Modifier.width(8.dp))
                    Text("Create New Local Calendar")
                }
            }

            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text("Aliases", fontWeight = FontWeight.Bold)
            }
            items(aliases.keys.toList()) { key ->
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 4.dp)) {
                    Text(
                        if (key.startsWith("@@")) key else "#$key",
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.width(80.dp)
                    )
                    Text("→", modifier = Modifier.padding(horizontal = 8.dp))
                    Text(aliases[key]?.joinToString(", ") ?: "", modifier = Modifier.weight(1f))
                    IconButton(onClick = {
                        scope.launch {
                            api.removeAlias(key)
                            reload()
                        }
                    }) { NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.error) }
                }
            }
            item {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    OutlinedTextField(
                        value = newAliasKey,
                        onValueChange = { newAliasKey = it },
                        label = { Text("Key (#tag or @@loc)") },
                        modifier = Modifier.weight(1f),
                        placeholder = { Text("#tag or @@loc") },
                    )
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = newAliasTags,
                        onValueChange = {
                            newAliasTags = it
                        },
                        label = { Text("Value(s)") },
                        placeholder = { Text("@@loc, #tag_b, !1") },
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = {
                        if (newAliasKey.isNotBlank() && newAliasTags.isNotBlank()) {
                            val tags = newAliasTags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                            scope.launch {
                                try {
                                    api.addAlias(newAliasKey.trimStart('#'), tags)
                                    newAliasKey = ""
                                    newAliasTags = ""
                                    reload()
                                    if (status.startsWith("Error")) status = ""
                                } catch (e: Exception) {
                                    status = "Error adding alias: ${e.message}"
                                }
                            }
                        }
                    }) { NfIcon(NfIcons.ADD) }
                }
                Spacer(Modifier.height(32.dp))
            }
        }
    }
}

@Composable
fun LocalCalendarEditor(
    cal: MobileCalendar,
    onUpdate: (String, String?) -> Unit,
    onDelete: () -> Unit,
    onExport: () -> Unit
) {
    var name by remember { mutableStateOf(cal.name) }
    var color by remember { mutableStateOf(cal.color) }

    // Debounce name updates or save on focus loss could be complex,
    // so we provide a "Save" icon button for explicit updates if name changes.
    val hasChanges = name != cal.name || color != cal.color
    val isDefault = cal.href == "local://default"

    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            // Row 1: Name and Actions
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("Name") },
                    modifier = Modifier.weight(1f),
                    singleLine = true
                )

                if (hasChanges) {
                    IconButton(onClick = { onUpdate(name, color) }) {
                        NfIcon(NfIcons.CHECK, 20.sp, MaterialTheme.colorScheme.primary)
                    }
                }

                IconButton(onClick = onExport) {
                    NfIcon(NfIcons.EXPORT, 20.sp, MaterialTheme.colorScheme.onSurfaceVariant)
                }

                if (!isDefault) {
                    IconButton(onClick = onDelete) {
                        NfIcon(NfIcons.DELETE, 20.sp, MaterialTheme.colorScheme.error)
                    }
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Row 2: Color Picker
            Text(
                "Color:",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            ColorPickerRow(
                selectedColor = color,
                onColorSelected = {
                    color = it
                    // Auto-save color changes immediately for better UX
                    onUpdate(name, it)
                }
            )
        }
    }
}

@Composable
fun ColorPickerRow(
    selectedColor: String?,
    onColorSelected: (String?) -> Unit
) {
    val colors = listOf(
        null to "None",
        "#FF4444" to "Red",
        "#FF8800" to "Orange",
        "#FFD700" to "Yellow",
        "#66BB6A" to "Green",
        "#42A5F5" to "Blue",
        "#AB47BC" to "Purple",
        "#9E9E9E" to "Gray"
    )

    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        colors.forEach { (hex, _) ->
            val isSelected = selectedColor == hex
            val colorVal = if (hex == null) Color.Transparent else parseHexColor(hex)

            Box(
                modifier = Modifier
                    .size(32.dp)
                    .background(colorVal, androidx.compose.foundation.shape.CircleShape)
                    .border(
                        width = if (isSelected) 2.dp else 1.dp,
                        color = if (isSelected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
                        shape = androidx.compose.foundation.shape.CircleShape
                    )
                    .clickable { onColorSelected(hex) },
                contentAlignment = Alignment.Center
            ) {
                if (hex == null) {
                    // slash for none
                    NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.onSurfaceVariant)
                } else if (isSelected) {
                    NfIcon(NfIcons.CHECK, 16.sp, Color.White)
                }
            }
        }
    }
}
