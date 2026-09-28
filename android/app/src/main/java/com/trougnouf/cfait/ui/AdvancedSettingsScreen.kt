// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.ui

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.res.stringResource
import androidx.core.content.FileProvider
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileFirstDayOfWeek
import com.trougnouf.cfait.R
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AdvancedSettingsScreen(
    api: CfaitMobile,
    tabPosition: String,
    actionBarPosition: String,
    tabAutoHide: Boolean,
    onTabPositionChange: (String) -> Unit,
    onActionBarPositionChange: (String) -> Unit,
    onTabAutoHideChange: (Boolean) -> Unit,
    onBack: () -> Unit
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var debugStatus by remember { mutableStateOf("") }
    var debugIsError by remember { mutableStateOf(false) }

    val maxDoneRootsState = remember { TextFieldState("20") }
    val maxDoneSubtasksState = remember { TextFieldState("5") }
    val trashRetentionState = remember { TextFieldState("14") }
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }
    var strikethroughCompleted by remember { mutableStateOf(false) }
    var showInlineDescriptions by remember { mutableStateOf(true) }
    var showQuickFilter by remember { mutableStateOf(true) }
    val quickFilterTermState = remember { TextFieldState("is:ready") }
    val quickFilterIconState = remember { TextFieldState("f0fa9") }

    var tlsClientCertPath by remember { mutableStateOf("") }
    var tlsClientKeyPath by remember { mutableStateOf("") }
    var useExternalStorage by remember { mutableStateOf(false) }
    var currentDataDirPath by remember { mutableStateOf("") }
    var showSwitchDialog by remember { mutableStateOf(false) }
    var pendingExternalStorage by remember { mutableStateOf(false) }

    var sortStandardByPriority by remember { mutableStateOf(false) }
    var pausedSortBehavior by remember { mutableStateOf("tiebreak") }
    var sortTiebreakRecent by remember { mutableStateOf(false) }
    var sortPreset by remember { mutableStateOf("Urgent > Ongoing > Due Soon") }
    val sortDaysState = remember { TextFieldState("30") }
    val urgentDaysState = remember { TextFieldState("1") }
    val urgentPrioState = remember { TextFieldState("1") }
    val defaultPriorityState = remember { TextFieldState("5") }
    val startGracePeriodDaysState = remember { TextFieldState("1") }
    var firstDayOfWeek by remember { mutableStateOf(MobileFirstDayOfWeek.MONDAY) }
    var showTaskGoalsInSidebar by remember { mutableStateOf(true) }
    var showCalendarsTab by remember { mutableStateOf(true) }
    var showTagsTab by remember { mutableStateOf(true) }
    var showLocationsTab by remember { mutableStateOf(true) }
    var showGoalsTab by remember { mutableStateOf(true) }
    var showJournalTab by remember { mutableStateOf(true) }
    val defaultDurationGoalMinsState = remember { TextFieldState("60") }
    var sessionsCountAsCompletions by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf("") }

    // getConfig/getCurrentDataDir do disk IO, so keep them off the main thread;
    // TextFieldState writes must happen on the main thread
    suspend fun reload() {
        try {
            var newMaxDoneRoots = ""
            var newMaxDoneSubtasks = ""
            var newTrashRetention = ""
            var newQuickFilterTerm = ""
            var newQuickFilterIcon = ""
            var newSortDays = ""
            var newUrgentDays = ""
            var newUrgentPrio = ""
            var newDefaultPriority = ""
            var newStartGracePeriodDays = ""
            var newDefaultDurationGoalMins = ""
            withContext(Dispatchers.IO) {
                val cfg = api.getConfig()
                newMaxDoneRoots = cfg.maxDoneRoots.toString()
                newMaxDoneSubtasks = cfg.maxDoneSubtasks.toString()
                newTrashRetention = cfg.trashRetention.toString()
                deleteEventsOnCompletion = cfg.deleteEventsOnCompletion
                strikethroughCompleted = cfg.strikethroughCompleted
                showInlineDescriptions = cfg.showInlineDescriptions
                showQuickFilter = cfg.showQuickFilter
                newQuickFilterTerm = cfg.quickFilterTerm
                newQuickFilterIcon = cfg.quickFilterIcon

                tlsClientCertPath = cfg.tlsClientCertPath ?: ""
                tlsClientKeyPath = cfg.tlsClientKeyPath ?: ""
                @Suppress("DEPRECATION")
                val extDir = context.externalMediaDirs.firstOrNull()?.absolutePath
                useExternalStorage = (extDir != null && cfg.dataDir == extDir)
                currentDataDirPath = try { api.getCurrentDataDir() } catch (_: Exception) { "" }

                sortStandardByPriority = cfg.sortStandardByPriority
                pausedSortBehavior = cfg.pausedSortBehavior
                sortTiebreakRecent = cfg.sortTiebreakRecent
                sortPreset = cfg.sortPreset
                newSortDays = cfg.sortCutoffDays?.toString() ?: ""
                newUrgentDays = cfg.urgentDays.toString()
                newUrgentPrio = cfg.urgentPrio.toString()
                newDefaultPriority = cfg.defaultPriority.toString()
                newStartGracePeriodDays = cfg.startGracePeriodDays.toString()
                firstDayOfWeek = cfg.firstDayOfWeek
                showTaskGoalsInSidebar = cfg.showTaskGoalsInSidebar
                showCalendarsTab = cfg.showCalendarsTab
                showTagsTab = cfg.showTagsTab
                showLocationsTab = cfg.showLocationsTab
                showGoalsTab = cfg.showGoalsTab
                showJournalTab = cfg.showJournalTab
                newDefaultDurationGoalMins = cfg.defaultDurationGoalMins.toString()
                sessionsCountAsCompletions = cfg.sessionsCountAsCompletions
            }
            maxDoneRootsState.setTextAndPlaceCursorAtEnd(newMaxDoneRoots)
            maxDoneSubtasksState.setTextAndPlaceCursorAtEnd(newMaxDoneSubtasks)
            trashRetentionState.setTextAndPlaceCursorAtEnd(newTrashRetention)
            quickFilterTermState.setTextAndPlaceCursorAtEnd(newQuickFilterTerm)
            quickFilterIconState.setTextAndPlaceCursorAtEnd(newQuickFilterIcon)
            sortDaysState.setTextAndPlaceCursorAtEnd(newSortDays)
            urgentDaysState.setTextAndPlaceCursorAtEnd(newUrgentDays)
            urgentPrioState.setTextAndPlaceCursorAtEnd(newUrgentPrio)
            defaultPriorityState.setTextAndPlaceCursorAtEnd(newDefaultPriority)
            startGracePeriodDaysState.setTextAndPlaceCursorAtEnd(newStartGracePeriodDays)
            defaultDurationGoalMinsState.setTextAndPlaceCursorAtEnd(newDefaultDurationGoalMins)
        } catch (e: Exception) {
            // Ignore on load
        }
    }

    LaunchedEffect(Unit) { reload() }

    // getConfig/saveConfig do disk IO, so keep them off the main thread
    suspend fun saveToDisk() {
        try {
            val maxDoneRootsStr = maxDoneRootsState.text.toString()
            val maxDoneSubtasksStr = maxDoneSubtasksState.text.toString()
            val trashRetentionStr = trashRetentionState.text.toString()
            val quickFilterTermStr = quickFilterTermState.text.toString()
            val quickFilterIconStr = quickFilterIconState.text.toString()
            val sortDaysStr = sortDaysState.text.toString()
            val urgentDaysStr = urgentDaysState.text.toString()
            val urgentPrioStr = urgentPrioState.text.toString()
            val defaultPriorityStr = defaultPriorityState.text.toString()
            val startGracePeriodDaysStr = startGracePeriodDaysState.text.toString()
            val defaultDurationGoalMinsStr = defaultDurationGoalMinsState.text.toString()
            withContext(Dispatchers.IO) {
                val cfg = api.getConfig()
                // Ensure at least one sidebar tab is visible
                val atLeastOneTab = showCalendarsTab || showTagsTab || showLocationsTab || showGoalsTab || showJournalTab
                val finalShowCalendarsTab = if (!showCalendarsTab && !atLeastOneTab) true else showCalendarsTab
                val finalShowTagsTab = if (!showTagsTab && !atLeastOneTab) true else showTagsTab
                val finalShowLocationsTab = if (!showLocationsTab && !atLeastOneTab) true else showLocationsTab
                val finalShowGoalsTab = if (!showGoalsTab && !atLeastOneTab) true else showGoalsTab
                val finalShowJournalTab = if (!showJournalTab && !atLeastOneTab) true else showJournalTab
                @Suppress("DEPRECATION")
                val newCfg = cfg.copy(
                    maxDoneRoots = maxDoneRootsStr.toUIntOrNull() ?: 20u,
                    maxDoneSubtasks = maxDoneSubtasksStr.toUIntOrNull() ?: 5u,
                    trashRetention = trashRetentionStr.toUIntOrNull() ?: 14u,
                    deleteEventsOnCompletion = deleteEventsOnCompletion,
                    strikethroughCompleted = strikethroughCompleted,
                    showInlineDescriptions = showInlineDescriptions,
                    showQuickFilter = showQuickFilter,
                    quickFilterTerm = quickFilterTermStr,
                    quickFilterIcon = quickFilterIconStr,

                    tlsClientCertPath = tlsClientCertPath.takeIf { it.isNotBlank() },
                    tlsClientKeyPath = tlsClientKeyPath.takeIf { it.isNotBlank() },
                    dataDir = if (useExternalStorage) context.externalMediaDirs.firstOrNull()?.absolutePath else null,

                    sortStandardByPriority = sortStandardByPriority,
                    pausedSortBehavior = pausedSortBehavior,
                    sortTiebreakRecent = sortTiebreakRecent,
                    sortPreset = sortPreset,
                    sortCutoffDays = sortDaysStr.toUIntOrNull(),
                    urgentDays = urgentDaysStr.toUIntOrNull() ?: 1u,
                    urgentPrio = urgentPrioStr.toUByteOrNull() ?: 1u,
                    defaultPriority = defaultPriorityStr.toUByteOrNull() ?: 5u,
                    startGracePeriodDays = startGracePeriodDaysStr.toUIntOrNull() ?: 1u,
                    firstDayOfWeek = firstDayOfWeek,
                    showTaskGoalsInSidebar = showTaskGoalsInSidebar,
                    showCalendarsTab = finalShowCalendarsTab,
                    showTagsTab = finalShowTagsTab,
                    showLocationsTab = finalShowLocationsTab,
                    showGoalsTab = finalShowGoalsTab,
                    showJournalTab = finalShowJournalTab,
                    defaultDurationGoalMins = defaultDurationGoalMinsStr.toUIntOrNull() ?: 60u,
                    sessionsCountAsCompletions = sessionsCountAsCompletions
                )
                api.saveConfig(newCfg)
            }
        } catch (e: Exception) {
            // Ignore save errors
        }
    }

    // Copy a picked content:// URI into app-private storage so the Rust side can
    // read it via std::fs without any storage permissions (scoped storage safe).
    fun copyUriToFilesDir(uri: Uri, destName: String): String? {
        return try {
            val dest = File(context.filesDir, destName)
            context.contentResolver.openInputStream(uri)?.use { input ->
                dest.outputStream().use { output -> input.copyTo(output) }
            }
            dest.absolutePath
        } catch (e: Exception) {
            null
        }
    }

    val certPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            val path = copyUriToFilesDir(uri, "tls_client_cert.pem")
            if (path != null) {
                tlsClientCertPath = path
                scope.launch { saveToDisk() }
            } else {
                status = context.getString(R.string.error_could_not_read_file)
            }
        }
    }

    val keyPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            val path = copyUriToFilesDir(uri, "tls_client_key.pem")
            if (path != null) {
                tlsClientKeyPath = path
                scope.launch { saveToDisk() }
            } else {
                status = context.getString(R.string.error_could_not_read_file)
            }
        }
    }

    // Save in the same coroutine as the navigation so the write lands before the screen is disposed
    val handleBack: () -> Unit = {
        scope.launch {
            saveToDisk()
            onBack()
        }
    }

    BackHandler { handleBack() }

    if (showSwitchDialog) {
        AlertDialog(
            onDismissRequest = { showSwitchDialog = false },
            title = { Text(stringResource(R.string.switch_data_dir_title)) },
            text = { Text(stringResource(R.string.switch_data_dir_text)) },
            confirmButton = {
                TextButton(onClick = {
                    @Suppress("DEPRECATION")
                    if (pendingExternalStorage && context.externalMediaDirs.firstOrNull() == null) {
                        showSwitchDialog = false
                        useExternalStorage = false
                        status = context.getString(R.string.error_external_storage_unavailable)
                        return@TextButton
                    }

                    showSwitchDialog = false
                    useExternalStorage = pendingExternalStorage

                    // Save before restarting so the new dataDir is persisted
                    scope.launch {
                        try {
                            saveToDisk()

                            val packageManager = context.packageManager
                            val intent = packageManager.getLaunchIntentForPackage(context.packageName)
                            val componentName = intent?.component
                            val mainIntent = Intent.makeRestartActivityTask(componentName)
                            context.startActivity(mainIntent)
                            Runtime.getRuntime().exit(0)
                        } catch (e: Exception) {
                            if (e is CancellationException) throw e
                            useExternalStorage = !pendingExternalStorage
                            status = context.getString(R.string.error_general, e.message ?: "")
                        }
                    }
                }) { Text(stringResource(R.string.switch_and_restart)) }
            },
            dismissButton = {
                TextButton(onClick = { showSwitchDialog = false }) { Text(stringResource(R.string.cancel)) }
            }
        )
    }

    // Pre-resolve strings that will be referenced from non-composable contexts (eg. inside coroutine)
    val exportExporting = stringResource(R.string.export_debug_status_exporting)
    val exportReady = stringResource(R.string.export_debug_status_ready)
    val exportFailedTemplate = stringResource(R.string.export_debug_status_failed)
    val exportShareTitle = stringResource(R.string.export_debug_share_title)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.advanced_settings_button)) },
                navigationIcon = {
                    IconButton(onClick = handleBack) { NfIcon(NfIcons.BACK, 20.sp) }
                }
            )
        }
    ) { padding ->
        val scrollState = rememberScrollState()
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize()
                .verticalScroll(scrollState)
        ) {
            // Collections Tab Section
            Text(
                stringResource(R.string.tab_position),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                FilterChip(
                    selected = tabPosition == "top",
                    onClick = { onTabPositionChange("top") },
                    label = { Text(stringResource(R.string.tab_pos_top)) },
                    modifier = Modifier.weight(1f)
                )
                FilterChip(
                    selected = tabPosition == "bottom",
                    onClick = { onTabPositionChange("bottom") },
                    label = { Text(stringResource(R.string.tab_pos_bottom)) },
                    modifier = Modifier.weight(1f)
                )
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(top = 8.dp, bottom = 16.dp)
            ) {
                Switch(checked = tabAutoHide, onCheckedChange = onTabAutoHideChange)
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.tab_auto_hide), style = MaterialTheme.typography.bodyMedium)
            }
            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Action Buttons Section
            Text(
                stringResource(R.string.action_bar_position),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                FilterChip(
                    selected = actionBarPosition == "top",
                    onClick = { onActionBarPositionChange("top") },
                    label = { Text(stringResource(R.string.tab_pos_top)) },
                    modifier = Modifier.weight(1f)
                )
                FilterChip(
                    selected = actionBarPosition == "bottom",
                    onClick = { onActionBarPositionChange("bottom") },
                    label = { Text(stringResource(R.string.tab_pos_bottom)) },
                    modifier = Modifier.weight(1f)
                )
            }
            Text(
                stringResource(R.string.action_bar_position_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )
            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Server Connection Additions (mTLS)
            Text(
                stringResource(R.string.server_connection),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            OutlinedButton(
                onClick = { certPicker.launch(arrayOf("*/*")) },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(stringResource(R.string.tls_client_cert_pick))
            }
            Text(
                text = tlsClientCertPath.ifEmpty { stringResource(R.string.tls_client_cert_none) },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            OutlinedButton(
                onClick = { keyPicker.launch(arrayOf("*/*")) },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp)
            ) {
                Text(stringResource(R.string.tls_client_key_pick))
            }
            Text(
                text = tlsClientKeyPath.ifEmpty { stringResource(R.string.tls_client_key_none) },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Sorting Rules
            Text(
                stringResource(R.string.sorting_and_visibility),
                fontWeight = FontWeight.Bold,
                fontSize = 20.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 8.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(checked = strikethroughCompleted, onCheckedChange = { strikethroughCompleted = it })
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.strikethrough_completed))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Switch(checked = showInlineDescriptions, onCheckedChange = { showInlineDescriptions = it })
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_inline_descriptions))
            }
            Spacer(Modifier.height(16.dp))
            
            Text(
                stringResource(R.string.settings_sorting),
                fontWeight = FontWeight.SemiBold,
                fontSize = 16.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 8.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(checked = sortStandardByPriority, onCheckedChange = { sortStandardByPriority = it })
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.sort_standard_by_priority_label))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Switch(checked = sortTiebreakRecent, onCheckedChange = { sortTiebreakRecent = it })
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.sort_tiebreak_recent))
            }
            
            Spacer(Modifier.height(16.dp))
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.settings_paused_tasks), modifier = Modifier.weight(1f))
                DropdownPicker(
                    label = "",
                    selected = pausedSortBehavior,
                    options = listOf(
                        "tiebreak" to stringResource(R.string.sort_paused_tiebreak),
                        "top" to stringResource(R.string.sort_paused_top),
                        "none" to stringResource(R.string.sort_paused_none)
                    ),
                    onSelect = { pausedSortBehavior = it },
                    modifier = Modifier.width(240.dp)
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.first_day_of_week), modifier = Modifier.weight(1f))
                DropdownPicker(
                    label = "",
                    selected = firstDayOfWeek,
                    options = listOf(
                        MobileFirstDayOfWeek.MONDAY to stringResource(R.string.monday),
                        MobileFirstDayOfWeek.SUNDAY to stringResource(R.string.sunday)
                    ),
                    onSelect = {
                        firstDayOfWeek = it
                        scope.launch { saveToDisk() }
                    },
                    modifier = Modifier.width(240.dp)
                )
            }

            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.sorting_preset_label), modifier = Modifier.weight(1f))
                DropdownPicker(
                    label = "",
                    selected = sortPreset,
                    options = listOf(
                        "Urgent > Ongoing > Due Soon" to stringResource(R.string.sort_preset_urgent_ongoing_due),
                        "Urgent > Due Soon > Ongoing" to stringResource(R.string.sort_preset_urgent_due_ongoing),
                        "Ongoing > Urgent > Due Soon" to stringResource(R.string.sort_preset_ongoing_urgent_due)
                    ),
                    onSelect = { sortPreset = it },
                    modifier = Modifier.width(240.dp)
                )
            }
            Text(
                stringResource(R.string.settings_sort_preset_explain),
                fontSize = 12.sp,
                color = Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            Text(
                stringResource(R.string.settings_urgent_and_timeframes),
                fontWeight = FontWeight.SemiBold,
                fontSize = 16.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 8.dp)
            )
            Text(
                stringResource(R.string.settings_urgent_definition),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.due_within_days), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    state = urgentDaysState,
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    lineLimits = TextFieldLineLimits.SingleLine
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.priority_le), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    state = urgentPrioState,
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    lineLimits = TextFieldLineLimits.SingleLine
                )
            }
            Text(
                stringResource(R.string.settings_urgent_explain),
                fontSize = 12.sp,
                color = Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            Text(
                stringResource(R.string.settings_timeframes_cutoffs),
                fontWeight = FontWeight.SemiBold,
                fontSize = 16.sp
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.priority_cutoff_days), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    state = sortDaysState,
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    lineLimits = TextFieldLineLimits.SingleLine
                )
            }
            Text(
                stringResource(R.string.settings_cutoff_explain),
                fontSize = 12.sp,
                color = Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 8.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.start_grace_days), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    state = startGracePeriodDaysState,
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    lineLimits = TextFieldLineLimits.SingleLine
                )
            }
            Text(
                stringResource(R.string.settings_start_grace_explain),
                fontSize = 12.sp,
                color = Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            Text(
                stringResource(R.string.settings_defaults),
                fontWeight = FontWeight.SemiBold,
                fontSize = 16.sp
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.default_priority_label), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    state = defaultPriorityState,
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    lineLimits = TextFieldLineLimits.SingleLine
                )
            }
            Text(
                stringResource(R.string.settings_default_prio_explain),
                fontSize = 12.sp,
                color = Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Sidebar Tabs Section
            Text(
                stringResource(R.string.sidebar_tabs),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                Switch(
                    checked = showCalendarsTab,
                    onCheckedChange = {
                        showCalendarsTab = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_calendars_tab))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                Switch(
                    checked = showTagsTab,
                    onCheckedChange = {
                        showTagsTab = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_tags_tab))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                Switch(
                    checked = showLocationsTab,
                    onCheckedChange = {
                        showLocationsTab = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_locations_tab))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                Switch(
                    checked = showGoalsTab,
                    onCheckedChange = {
                        showGoalsTab = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_goals_tab))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                Switch(
                    checked = showJournalTab,
                    onCheckedChange = {
                        showJournalTab = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_journal_tab))
            }

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Display Limits Section
            Text(
                text = stringResource(R.string.display_limits),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )

            OutlinedTextField(
                state = maxDoneRootsState,
                label = { Text(stringResource(R.string.max_completed_tasks_root)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            Text(
                stringResource(R.string.max_completed_tasks_root_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            OutlinedTextField(
                state = maxDoneSubtasksState,
                label = { Text(stringResource(R.string.max_completed_subtasks)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            Text(
                stringResource(R.string.max_completed_subtasks_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            Text(
                stringResource(R.string.quick_filter_title),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = showQuickFilter,
                    onCheckedChange = { showQuickFilter = it }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.quick_filter_show_button))
            }
            OutlinedTextField(
                state = quickFilterTermState,
                label = { Text(stringResource(R.string.quick_filter_search_term)) },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            OutlinedTextField(
                state = quickFilterIconState,
                label = { Text(stringResource(R.string.quick_filter_icon)) },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 16.dp),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Calendar Integration Section
            Text(
                stringResource(R.string.calendar_integration),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = deleteEventsOnCompletion,
                    onCheckedChange = { deleteEventsOnCompletion = it }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.delete_calendar_events_on_completion_label))
            }
            Text(
                stringResource(R.string.events_deleted_on_task_delete),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Data Management Section
            Text(
                stringResource(R.string.data_management),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = useExternalStorage,
                    onCheckedChange = { isChecked ->
                        pendingExternalStorage = isChecked
                        showSwitchDialog = true
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.use_external_storage_label))
            }
            Text(
                stringResource(R.string.use_external_storage_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 4.dp)
            )
            if (currentDataDirPath.isNotEmpty()) {
                Text(
                    stringResource(R.string.use_external_storage_path, currentDataDirPath),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(bottom = 16.dp)
                )
            }
            OutlinedTextField(
                state = trashRetentionState,
                label = { Text(stringResource(R.string.trash_retention_days_label)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            Text(
                stringResource(R.string.trash_retention_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 24.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Goals Section (Moved from SettingsScreen)
            Text(
                stringResource(R.string.goals),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 16.dp)) {
                Switch(
                    checked = showTaskGoalsInSidebar,
                    onCheckedChange = {
                        showTaskGoalsInSidebar = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_task_goals_in_sidebar))
            }
            OutlinedTextField(
                state = defaultDurationGoalMinsState,
                label = { Text(stringResource(R.string.implicit_goal_duration)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                lineLimits = TextFieldLineLimits.SingleLine
            )
            Text(
                stringResource(R.string.implicit_goal_duration_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = sessionsCountAsCompletions,
                    onCheckedChange = {
                        sessionsCountAsCompletions = it
                        scope.launch { saveToDisk() }
                    }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.sessions_count_as_completions))
            }

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Debug Section (Moved from SettingsScreen)
            Text(
                stringResource(R.string.export_debug_share_title),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Text(
                stringResource(R.string.debug_export_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Button(
                onClick = {
                    scope.launch {
                        try {
                            debugIsError = false
                            debugStatus = exportExporting
                            // createDebugExport and the file copy are disk IO
                            val destFile = withContext(Dispatchers.IO) {
                                val zipPath = api.createDebugExport()
                                val sourceFile = File(zipPath)
                                val dest = File(context.cacheDir, "cfait_debug_export.zip")
                                sourceFile.copyTo(dest, overwrite = true)
                                dest
                            }

                            val uri = FileProvider.getUriForFile(
                                context,
                                "${context.packageName}.fileprovider",
                                destFile
                            )

                            val intent = Intent(Intent.ACTION_SEND).apply {
                                type = "application/zip"
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }

                            val shareIntent = Intent.createChooser(intent, exportShareTitle)
                            context.startActivity(shareIntent)
                            debugIsError = false
                            debugStatus = exportReady
                        } catch (e: Exception) {
                            if (e is CancellationException) throw e
                            debugIsError = true
                            debugStatus = try {
                                String.format(exportFailedTemplate, e.message ?: e.toString())
                            } catch (_: Exception) {
                                // Fallback if formatting fails
                                "${exportFailedTemplate} ${e.message ?: e.toString()}"
                            }
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer
                )
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    NfIcon(NfIcons.ARCHIVE_ARROW_UP, 16.sp)
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.export))
                }
            }

            if (debugStatus.isNotEmpty()) {
                Text(
                    text = debugStatus,
                    color = if (debugIsError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(top = 8.dp),
                    style = MaterialTheme.typography.bodySmall
                )
            }

            if (status.isNotEmpty()) {
                Text(
                    text = status,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}
