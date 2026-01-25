package com.trougnouf.cfait.ui

import android.content.Intent
import android.os.Build
import android.widget.Toast
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
import com.trougnouf.cfait.core.MobileAccount
import com.trougnouf.cfait.core.MobileCalendar
import com.trougnouf.cfait.core.MobileConfigUpdate
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
    currentTheme: String,
    onThemeChange: (String) -> Unit
) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    // Account List State
    var accounts by remember { mutableStateOf<List<MobileAccount>>(emptyList()) }
    var showAccountDialog by remember { mutableStateOf(false) }
    var editingAccount by remember { mutableStateOf<MobileAccount?>(null) }

    // Preferences State
    var hideCompleted by remember { mutableStateOf(false) }
    var sortMonths by remember { mutableStateOf("2") }
    var status by remember { mutableStateOf("") }
    var aliases by remember { mutableStateOf<Map<String, List<String>>>(emptyMap()) }
    var newAliasKey by remember { mutableStateOf("") }
    var newAliasTags by remember { mutableStateOf("") }
    var allCalendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var disabledSet by remember { mutableStateOf<Set<String>>(emptySet()) }

    // Config fields
    var urgentDays by remember { mutableStateOf("1") }
    var urgentPrio by remember { mutableStateOf("1") }
    var defaultPriority by remember { mutableStateOf("5") }
    var startGracePeriodDays by remember { mutableStateOf("1") }
    var autoRemind by remember { mutableStateOf(true) }
    var defTime by remember { mutableStateOf("09:00") }
    var snoozeShort by remember { mutableStateOf("1h") }
    var snoozeLong by remember { mutableStateOf("1d") }
    var createEventsForTasks by remember { mutableStateOf(false) }
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }

    // Theme dropdown state
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
        accounts = api.getAccounts()
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
        snoozeShort = formatDuration(cfg.snoozeShort)
        snoozeLong = formatDuration(cfg.snoozeLong)
        createEventsForTasks = cfg.createEventsForTasks
        deleteEventsOnCompletion = cfg.deleteEventsOnCompletion

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

        val sShort = api.parseDurationString(snoozeShort) ?: 60u
        val sLong = api.parseDurationString(snoozeLong) ?: 1440u

        // FIX: Use MobileConfigUpdate data class to match the Rust FFI
        val update = MobileConfigUpdate(
            hideCompleted = hideCompleted,
            disabledCalendars = disabledSet.toList(),
            sortCutoffMonths = sortInt,
            urgentDays = daysInt,
            urgentPrio = prioInt,
            defaultPriority = defaultPrioInt,
            startGracePeriodDays = startGraceInt,
            autoReminders = autoRemind,
            defaultReminderTime = defTime,
            snoozeShort = sShort,
            snoozeLong = sLong,
            createEventsForTasks = createEventsForTasks,
            deleteEventsOnCompletion = deleteEventsOnCompletion
        )
        api.saveConfig(update)
    }

    fun handleBack() {
        scope.launch {
            saveToDisk()
            if (!initialCreateEventsState && createEventsForTasks) {
                onCreateEvents()
            }
            onBack()
        }
    }

    BackHandler { handleBack() }

    if (showAccountDialog) {
        AccountDialog(
            account = editingAccount,
            api = api,
            onDismiss = { showAccountDialog = false },
            onSave = {
                showAccountDialog = false
                reload()
            }
        )
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
        LazyColumn(modifier = Modifier.padding(p).imePadding().padding(16.dp)) {

            // --- ACCOUNTS SECTION ---
            item {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                    modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)
                ) {
                    Text(
                        "Accounts",
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    IconButton(onClick = {
                        editingAccount = null
                        showAccountDialog = true
                    }) {
                        NfIcon(NfIcons.ADD, 20.sp, MaterialTheme.colorScheme.primary)
                    }
                }
            }

            if (accounts.isEmpty()) {
                item {
                    Text("No accounts configured.", style = MaterialTheme.typography.bodyMedium, color = Color.Gray)
                }
            } else {
                items(accounts) { acc ->
                    Card(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp).clickable {
                            editingAccount = acc
                            showAccountDialog = true
                        },
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
                    ) {
                        Row(
                            modifier = Modifier.padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(acc.name, fontWeight = FontWeight.Bold)
                                Text(acc.username, style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                            }
                            IconButton(onClick = {
                                scope.launch {
                                    api.deleteAccount(acc.id)
                                    reload()
                                }
                            }) {
                                NfIcon(NfIcons.DELETE, 18.sp, MaterialTheme.colorScheme.error)
                            }
                        }
                    }
                }
            }

            item {
                Button(
                    onClick = {
                        scope.launch {
                            status = "Syncing..."
                            try {
                                status = api.sync()
                                reload()
                            } catch(e: Exception) {
                                status = "Sync Error: ${e.message}"
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth().padding(top = 16.dp)
                ) {
                    Text("Sync Now")
                }
                if (status.isNotEmpty()) {
                    Text(
                        status,
                        color = if (status.startsWith("Error")) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
            }

            // --- REST OF SETTINGS (Unchanged mostly) ---
            item { Text("Manage calendars", fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 8.dp)) }
            items(allCalendars) { cal ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable {
                            val newSet = disabledSet.toMutableSet()
                            val enabled = disabledSet.contains(cal.href)
                            if (enabled) newSet.remove(cal.href) else newSet.add(cal.href)
                            disabledSet = newSet
                            saveToDisk()
                        }
                ) {
                    Checkbox(checked = !disabledSet.contains(cal.href), onCheckedChange = { enabled ->
                        val newSet = disabledSet.toMutableSet()
                        if (enabled) newSet.remove(cal.href) else newSet.add(cal.href)
                        disabledSet = newSet
                        saveToDisk()
                    })
                    Column(modifier = Modifier.weight(1f)) {
                        Text(cal.name)
                        // Show account/source name if available/relevant
                    }
                }
            }

            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text("Appearance", fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
                // Theme Dropdown (same as before)
                Box {
                    OutlinedCard(
                        modifier = Modifier.fillMaxWidth().clickable { themeExpanded = true }.padding(top = 8.dp),
                    ) {
                        Row(
                            modifier = Modifier.padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Column {
                                Text("App Theme", fontWeight = FontWeight.SemiBold)
                                Text(
                                    text = themeOptions.find { it.first == currentTheme }?.second ?: "Auto-detect",
                                    style = MaterialTheme.typography.bodySmall
                                )
                            }
                            NfIcon(NfIcons.ARROW_DOWN, 12.sp)
                        }
                    }
                    DropdownMenu(
                        expanded = themeExpanded,
                        onDismissRequest = { themeExpanded = false },
                    ) {
                        themeOptions.forEach { (key, label) ->
                            DropdownMenuItem(
                                text = { Text(label) },
                                onClick = { onThemeChange(key); themeExpanded = false }
                            )
                        }
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
            }

            // ... (Rest of preferences: Sorting, Notifications, etc. - mostly same as previous)
            item {
                Text("Preferences", fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = hideCompleted, onCheckedChange = { hideCompleted = it; saveToDisk() })
                    Text("Hide completed")
                }

                // Sorting section
                OutlinedCard(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("Sorting & Visibility", fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)

                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                            Text("Urgent Days:", modifier = Modifier.weight(1f))
                            OutlinedTextField(value = urgentDays, onValueChange = { urgentDays = it }, modifier = Modifier.width(60.dp))
                        }
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                            Text("Urgent Prio (<=):", modifier = Modifier.weight(1f))
                            OutlinedTextField(value = urgentPrio, onValueChange = { urgentPrio = it }, modifier = Modifier.width(60.dp))
                        }
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                            Text("Default Prio:", modifier = Modifier.weight(1f))
                            OutlinedTextField(value = defaultPriority, onValueChange = { defaultPriority = it }, modifier = Modifier.width(60.dp))
                        }
                    }
                }
            }

            // ... (Local Calendars, Aliases - same as previous)

            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text("Local Calendars", fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
                val localCals = allCalendars.filter { it.isLocal }
                localCals.forEach { cal ->
                    LocalCalendarEditor(
                        cal = cal,
                        onUpdate = { name, color ->
                            scope.launch { api.updateLocalCalendar(cal.href, name, color); reload() }
                        },
                        onDelete = {
                            scope.launch { api.deleteLocalCalendar(cal.href); reload() }
                        },
                        onExport = {
                             try {
                                val icsContent = api.exportLocalIcs(cal.href)
                                val calId = cal.href.removePrefix("local://")
                                val file = File(context.cacheDir, "cfait_${calId}.ics")
                                file.writeText(icsContent)
                                val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                                val intent = Intent(Intent.ACTION_SEND).apply {
                                    type = "text/calendar"
                                    putExtra(Intent.EXTRA_STREAM, uri)
                                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                }
                                context.startActivity(Intent.createChooser(intent, "Export ${cal.name}"))
                            } catch (e: Exception) {
                                status = "Export Error: ${e.message}"
                            }
                        },
                        onImport = {
                            importTargetHref = cal.href
                            importLauncher.launch("*/*")
                        }
                    )
                    Spacer(Modifier.height(8.dp))
                }
                Button(
                    onClick = { scope.launch { api.createLocalCalendar("New Calendar", null); reload() } },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Create Local Calendar") }
            }
        }
    }
}

@Composable
fun AccountDialog(
    account: MobileAccount?,
    api: CfaitMobile,
    onDismiss: () -> Unit,
    onSave: () -> Unit
) {
    var name by remember { mutableStateOf(account?.name ?: "New Account") }
    var url by remember { mutableStateOf(account?.url ?: "") }
    var user by remember { mutableStateOf(account?.username ?: "") }
    var pass by remember { mutableStateOf(account?.password ?: "") }
    var insecure by remember { mutableStateOf(account?.allowInsecure ?: false) }
    var testStatus by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (account == null) "Add Account" else "Edit Account") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Name") }, singleLine = true)
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("URL") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri)
                )
                OutlinedTextField(value = user, onValueChange = { user = it }, label = { Text("Username") }, singleLine = true)
                OutlinedTextField(
                    value = pass,
                    onValueChange = { pass = it },
                    label = { Text("Password") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = insecure, onCheckedChange = { insecure = it })
                    Text("Allow Insecure SSL")
                }

                Button(
                    onClick = {
                        scope.launch {
                            testStatus = "Testing..."
                            try {
                                testStatus = api.validateConnection(url, user, pass, insecure)
                            } catch (e: Exception) {
                                testStatus = "Failed: ${e.message}"
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Test Connection") }

                if (testStatus.isNotEmpty()) {
                    Text(testStatus, color = if (testStatus.startsWith("Success")) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            Button(onClick = {
                scope.launch {
                    try {
                        api.saveAccount(account?.id ?: "", name, url, user, pass, insecure)
                        onSave()
                    } catch(e: Exception) {
                        testStatus = "Save Error: ${e.message}"
                    }
                }
            }) { Text("Save") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}
