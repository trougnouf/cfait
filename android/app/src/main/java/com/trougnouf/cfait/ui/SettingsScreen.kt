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
import androidx.compose.ui.res.stringResource
import androidx.core.content.FileProvider
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileCalendar
import kotlinx.coroutines.launch
import java.io.File

/* busyMessages moved into the composable so strings can be resolved with stringResource()
   (must be inside a @Composable function). */

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

    // State maintained purely for saving without overwriting backend values
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }

    var themeExpanded by remember { mutableStateOf(false) }
    // Use localized labels for theme options so they appear translated on Android.
    val themeOptions = remember {
        val list = mutableListOf(
            "auto" to stringResource(R.string.theme_auto_detect),
            "light" to stringResource(R.string.theme_light),
            "dark" to stringResource(R.string.theme_dark)
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            list.add("dynamic_light" to stringResource(R.string.theme_dynamic_light))
            list.add("dynamic_dark" to stringResource(R.string.theme_dynamic_dark))
        }
        list
    }

    var initialCreateEventsState by remember { mutableStateOf(false) }
    var isInitialLoad by remember { mutableStateOf(true) }

    // Localized busy messages (resolved inside the composable)
    val busyMessages = listOf(
        stringResource(R.string.busy_processing_stuff),
        stringResource(R.string.busy_brb),
        stringResource(R.string.busy_be_right_back),
        stringResource(R.string.busy_working_on_things),
        stringResource(R.string.busy_loading),
        stringResource(R.string.busy_afk),
        stringResource(R.string.busy_processing_things),
        stringResource(R.string.busy_reticulating_splines),
        stringResource(R.string.busy_swapping_time_streams),
        stringResource(R.string.busy_defragmenting_memories),
        stringResource(R.string.busy_sorting_mental_baggage),
        stringResource(R.string.busy_getting_things_done)
    )

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
                        status = context.getString(R.string.error_could_not_read_file)
                    }
                } catch (e: Exception) {
                    status = context.getString(R.string.import_error, e.message ?: "")
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

        val trashInt = cfg.trashRetention
        val maxRootsInt = cfg.maxDoneRoots.toUInt()
        val maxSubtasksInt = cfg.maxDoneSubtasks.toUInt()

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
            status = context.getString(R.string.connecting)
            try {
                saveToDisk()
                status = api.connect(url, user, pass, insecure)
                reload()
            } catch (e: Exception) {
                status = context.getString(R.string.connection_failed, e.message ?: "")
            }
        }
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

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings)) },
                navigationIcon = {
                    IconButton(onClick = { handleBack() }) { NfIcon(NfIcons.BACK, 20.sp) }
                },
                actions = { IconButton(onClick = onHelp) { NfIcon(NfIcons.HELP, 24.sp) } }
            )
        }
    ) { p ->
        LazyColumn(
            modifier = Modifier
                .padding(p)
                .imePadding()
                .fillMaxSize(),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            // 1. Connection Section
            item {
                Text(
                    stringResource(R.string.server_connection),
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text(androidx.compose.ui.res.stringResource(R.string.caldav_url)) },
                    modifier = Modifier.fillMaxWidth(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        autoCorrect = false,
                        capitalization = KeyboardCapitalization.None
                    ),
                    singleLine = true
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = user,
                    onValueChange = { user = it },
                    label = { Text(androidx.compose.ui.res.stringResource(R.string.username)) },
                    modifier = Modifier.fillMaxWidth(),
                    keyboardOptions = KeyboardOptions(
                        autoCorrect = false,
                        capitalization = KeyboardCapitalization.None
                    ),
                    singleLine = true
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = pass,
                    onValueChange = { pass = it },
                    label = { Text(androidx.compose.ui.res.stringResource(R.string.password)) },
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        autoCorrect = false
                    ),
                    singleLine = true
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = insecure, onCheckedChange = { insecure = it })
                    Text(androidx.compose.ui.res.stringResource(R.string.allow_insecure_ssl))
                }
                Button(
                    onClick = { saveAndConnect() },
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp)
                ) { Text(androidx.compose.ui.res.stringResource(R.string.save_and_connect)) }
                if (status.isNotEmpty()) {
                    Text(
                        status,
                        color = if (status.startsWith("Connection failed")) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }

            // 2. Appearance
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.appearance),
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )

                Box {
                    // Theme selector card (localized)
                    OutlinedCard(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { themeExpanded = true },
                    ) {
                        Row(
                            modifier = Modifier.padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Column {
                                Text(
                                    androidx.compose.ui.res.stringResource(R.string.app_theme),
                                    fontWeight = FontWeight.SemiBold
                                )
                                Text(
                                    text = themeOptions.find { it.first == currentTheme }?.second
                                        ?: androidx.compose.ui.res.stringResource(R.string.theme_auto_detect),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            NfIcon(NfIcons.ARROW_DOWN, 12.sp)
                        }
                    }

                    DropdownMenu(
                        expanded = themeExpanded,
                        onDismissRequest = { themeExpanded = false },
                        modifier = Modifier.fillMaxWidth(0.9f)
                    ) {
                        themeOptions.forEach { (key, label) ->
                            DropdownMenuItem(
                                text = { Text(label) },
                                onClick = {
                                    onThemeChange(key)
                                    themeExpanded = false
                                },
                                leadingIcon = if (currentTheme == key) {
                                    { NfIcon(NfIcons.CHECK, 16.sp) }
                                } else null
                            )
                        }
                    }

                    // Language selector (persistent to Android SharedPreferences).
                    var languageExpanded by remember { mutableStateOf(false) }
                    var selectedLanguage by remember { mutableStateOf<String?>(null) }

                    // Dynamically build options from the Rust backend using native Java Locale resolution
                    val systemDefaultLabel = stringResource(R.string.language_system)
                    val languageOptions = remember(systemDefaultLabel) {
                        val options = mutableListOf<Pair<String?, String>>(
                            null to systemDefaultLabel
                        )
                        val locales = try {
                            api.getAvailableLocales()
                        } catch (e: Exception) {
                            emptyList<String>()
                        }
                        for (code in locales) {
                            val loc = java.util.Locale.forLanguageTag(code)
                            // Gets the native name (e.g. "Français", "Deutsch") and capitalizes it
                            val nativeName = loc.getDisplayName(loc)
                            val capitalized = nativeName.replaceFirstChar {
                                if (it.isLowerCase()) it.titlecase(loc) else it.toString()
                            }
                            options.add(code to capitalized)
                        }
                        options
                    }

                    // Load persisted choice when reloading settings
                    LaunchedEffect(Unit) {
                        val prefs = context.getSharedPreferences("cfait_prefs", android.content.Context.MODE_PRIVATE)
                        val savedLang = prefs.getString("language", null)
                        selectedLanguage = if (savedLang == "auto") null else savedLang
                    }

                    OutlinedCard(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 12.dp)
                            .clickable { languageExpanded = true },
                    ) {
                        Row(
                            modifier = Modifier.padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Column {
                                Text(
                                    stringResource(R.string.language),
                                    fontWeight = FontWeight.SemiBold
                                )
                                Text(
                                    // Display the pretty label for the selected code
                                    text = languageOptions.find { it.first == selectedLanguage }?.second
                                        ?: systemDefaultLabel,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            NfIcon(NfIcons.ARROW_DOWN, 12.sp)
                        }
                    }

                    DropdownMenu(
                        expanded = languageExpanded,
                        onDismissRequest = { languageExpanded = false },
                        modifier = Modifier.fillMaxWidth(0.9f)
                    ) {
                        languageOptions.forEach { (code, label) ->
                            DropdownMenuItem(
                                text = { Text(label) },
                                onClick = {
                                    val prefs = context.getSharedPreferences(
                                        "cfait_prefs",
                                        android.content.Context.MODE_PRIVATE
                                    )
                                    val saveVal = code ?: "auto"
                                    prefs.edit().putString("language", saveVal).apply()
                                    selectedLanguage = code
                                    languageExpanded = false

                                    // Update Rust backend
                                    api.setLocale(code ?: java.util.Locale.getDefault().language)

                                    // Update Android UI (safely recreates the Activity)
                                    if (code != null) {
                                        androidx.appcompat.app.AppCompatDelegate.setApplicationLocales(
                                            androidx.core.os.LocaleListCompat.forLanguageTags(code)
                                        )
                                    } else {
                                        androidx.appcompat.app.AppCompatDelegate.setApplicationLocales(
                                            androidx.core.os.LocaleListCompat.getEmptyLocaleList()
                                        )
                                    }
                                },
                                leadingIcon = if (selectedLanguage == code) {
                                    { NfIcon(NfIcons.CHECK, 16.sp) }
                                } else null
                            )
                        }
                    }
                }
            }

            // 3. Manage Collections
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.manage_collections),
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
            }
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
                    Text(cal.name, modifier = Modifier.weight(1f))
                }
            }

            // 4. Preferences
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.preferences),
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = hideCompleted, onCheckedChange = {
                        hideCompleted = it
                        saveToDisk()
                    })
                    Text(androidx.compose.ui.res.stringResource(R.string.hide_completed_and_canceled_tasks))
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = autoRemind, onCheckedChange = { autoRemind = it })
                    Text(androidx.compose.ui.res.stringResource(R.string.auto_remind_on_due_start))
                }
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text(
                        androidx.compose.ui.res.stringResource(R.string.default_time_label),
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedTextField(
                        value = defTime,
                        onValueChange = { defTime = it },
                        modifier = Modifier.width(100.dp),
                        singleLine = true
                    )
                }

            }

            // 5. Background Sync
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.background_sync),
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        androidx.compose.ui.res.stringResource(R.string.sync_interval_label),
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedTextField(
                        value = autoRefresh,
                        onValueChange = { autoRefresh = it },
                        modifier = Modifier.width(80.dp),
                        singleLine = true
                    )
                }
                Text(
                    "Note: Android enforces a minimum interval of 15 minutes.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 4.dp, top = 4.dp, bottom = 8.dp)
                )

                // Battery Optimization Warning
                val powerManager =
                    context.getSystemService(android.content.Context.POWER_SERVICE) as? android.os.PowerManager
                val isIgnoringBatteryOptimizations =
                    powerManager?.isIgnoringBatteryOptimizations(context.packageName) ?: true

                if (!isIgnoringBatteryOptimizations) {
                    Button(
                        onClick = {
                            try {
                                val intent =
                                    android.content.Intent(android.provider.Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
                                context.startActivity(intent)
                            } catch (e: Exception) {
                                status = context.getString(R.string.cannot_open_battery_settings)
                            }
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = MaterialTheme.colorScheme.tertiaryContainer,
                            contentColor = MaterialTheme.colorScheme.onTertiaryContainer
                        )
                    ) {
                        Text(androidx.compose.ui.res.stringResource(R.string.disable_battery_optimizations))
                    }
                    Text(
                        "Allow cfait to run in the background without restrictions for reliable synchronization and alarms.",
                        fontSize = 12.sp,
                        color = androidx.compose.ui.graphics.Color.Gray,
                        modifier = Modifier.padding(start = 4.dp, top = 4.dp)
                    )
                }
            }

            // 6. Calendar Integration
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.calendar_integration),
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
                    Text(androidx.compose.ui.res.stringResource(R.string.create_calendar_events_for_tasks_with_dates))
                }
                Text(
                    "Events will be retroactively created. Use +cal/-cal per task to override.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 12.dp, bottom = 8.dp)
                )

                Button(
                    onClick = {
                        createEventsForTasks = false
                        saveToDisk()
                        onDeleteEvents()
                    },
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
                        Text(androidx.compose.ui.res.stringResource(R.string.delete_all_calendar_events))
                    }
                }

                Text(
                    "This is fully reversible. Simply toggle 'Create calendar events' off and back on to recreate them.",
                    fontSize = 12.sp,
                    color = androidx.compose.ui.graphics.Color.Gray,
                    modifier = Modifier.padding(start = 4.dp, top = 4.dp)
                )
            }

            // 6. Local collections
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.local_collections),
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )
            }
            items(allCalendars.filter { it.isLocal }) { cal ->
                LocalCalendarEditor(
                    cal = cal,
                    onUpdate = { name, color ->
                        scope.launch {
                            try {
                                api.updateLocalCalendar(cal.href, name, color)
                                reload()
                            } catch (e: Exception) {
                                status = context.getString(R.string.error, e.message ?: "")
                            }
                        }
                    },
                    onDelete = {
                        scope.launch {
                            try {
                                api.deleteLocalCalendar(cal.href)
                                reload()
                            } catch (e: Exception) {
                                status = context.getString(R.string.error, e.message ?: "")
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
                            status = context.getString(R.string.export_error, e.message ?: "")
                        }
                    },
                    onImport = {
                        importTargetHref = cal.href
                        importLauncher.launch("*/*")
                    }
                )
                Spacer(modifier = Modifier.height(12.dp))
            }
            item {
                Button(
                    onClick = {
                        scope.launch {
                            try {
                                api.createLocalCalendar("New Calendar", null)
                                reload()
                            } catch (e: Exception) {
                                status = context.getString(R.string.error, e.message ?: "")
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
                    Text(androidx.compose.ui.res.stringResource(R.string.create_new_local_calendar))
                }
            }

            // 7. Aliases
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text(
                    stringResource(R.string.aliases),
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
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
                        label = { Text(androidx.compose.ui.res.stringResource(R.string.alias_key_label)) },
                        modifier = Modifier.weight(1f),
                        placeholder = { Text(androidx.compose.ui.res.stringResource(R.string.placeholder_key_tag)) },
                    )
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = newAliasTags,
                        onValueChange = { newAliasTags = it },
                        label = { Text(androidx.compose.ui.res.stringResource(R.string.alias_value_label)) },
                        placeholder = { Text(androidx.compose.ui.res.stringResource(R.string.placeholder_values)) },
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
                                    status = context.getString(R.string.error_adding_alias, e.message ?: "")
                                }
                            }
                        }
                    }) { NfIcon(NfIcons.ADD) }
                }
            }

            // 8. Advanced Settings
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Button(onClick = onAdvanced, modifier = Modifier.fillMaxWidth()) {
                    Text(androidx.compose.ui.res.stringResource(R.string.advanced_settings_button))
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
    onExport: () -> Unit,
    onImport: () -> Unit
) {
    var name by remember { mutableStateOf(cal.name) }
    var color by remember { mutableStateOf(cal.color) }

    val hasChanges = name != cal.name || color != cal.color
    val isDefault = cal.href == "local://default"

    Card(
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.name_label)) },
                    modifier = Modifier.weight(1f),
                    singleLine = true
                )

                if (hasChanges) {
                    IconButton(onClick = { onUpdate(name, color) }) {
                        NfIcon(NfIcons.CHECK, 20.sp, MaterialTheme.colorScheme.primary)
                    }
                }

                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    IconButton(onClick = onExport) {
                        NfIcon(NfIcons.EXPORT, 20.sp, MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Text(
                        stringResource(R.string.export),
                        style = MaterialTheme.typography.labelSmall,
                        fontSize = 8.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    IconButton(onClick = onImport) {
                        NfIcon(NfIcons.IMPORT, 20.sp, MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Text(
                        stringResource(R.string.import_action),
                        style = MaterialTheme.typography.labelSmall,
                        fontSize = 8.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                if (!isDefault) {
                    IconButton(onClick = onDelete) {
                        NfIcon(NfIcons.DELETE, 20.sp, MaterialTheme.colorScheme.error)
                    }
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            Text(
                stringResource(R.string.color_label),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            ColorPickerRow(
                selectedColor = color,
                onColorSelected = {
                    color = it
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
                    NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.onSurfaceVariant)
                } else if (isSelected) {
                    NfIcon(NfIcons.CHECK, 16.sp, Color.White)
                }
            }
        }
    }
}
