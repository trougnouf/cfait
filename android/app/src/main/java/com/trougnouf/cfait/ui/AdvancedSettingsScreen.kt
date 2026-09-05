// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/AdvancedSettingsScreen.kt
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
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.res.stringResource
import androidx.core.content.FileProvider
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.R
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
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

    var maxDoneRoots by remember { mutableStateOf("20") }
    var maxDoneSubtasks by remember { mutableStateOf("5") }
    var trashRetention by remember { mutableStateOf("14") }
    var deleteEventsOnCompletion by remember { mutableStateOf(false) }
    var strikethroughCompleted by remember { mutableStateOf(false) }
    var showInlineDescriptions by remember { mutableStateOf(true) }
    var showOngoingNotifications by remember { mutableStateOf(true) }
    var showQuickFilter by remember { mutableStateOf(true) }
    var quickFilterTerm by remember { mutableStateOf("is:ready") }
    var quickFilterIcon by remember { mutableStateOf("f0fa9") }

    var tlsClientCertPath by remember { mutableStateOf("") }
    var tlsClientKeyPath by remember { mutableStateOf("") }

    var sortStandardByPriority by remember { mutableStateOf(false) }
    var pausedSortBehavior by remember { mutableStateOf("tiebreak") }
    var sortTiebreakRecent by remember { mutableStateOf(false) }
    var sortPreset by remember { mutableStateOf("Urgent > Ongoing > Due Soon") }
    var sortDays by remember { mutableStateOf("30") }
    var urgentDays by remember { mutableStateOf("1") }
    var urgentPrio by remember { mutableStateOf("1") }
    var defaultPriority by remember { mutableStateOf("5") }
    var startGracePeriodDays by remember { mutableStateOf("1") }
    var firstDayOfWeek by remember { mutableStateOf(com.trougnouf.cfait.core.MobileFirstDayOfWeek.MONDAY) }
    var showTaskGoalsInSidebar by remember { mutableStateOf(true) }
    var defaultDurationGoalMins by remember { mutableStateOf("60") }
    var sessionsCountAsCompletions by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf("") }

    fun reload() {
        try {
            val cfg = api.getConfig()
            maxDoneRoots = cfg.maxDoneRoots.toString()
            maxDoneSubtasks = cfg.maxDoneSubtasks.toString()
            trashRetention = cfg.trashRetention.toString()
            deleteEventsOnCompletion = cfg.deleteEventsOnCompletion
            strikethroughCompleted = cfg.strikethroughCompleted
            showInlineDescriptions = cfg.showInlineDescriptions
            showOngoingNotifications = cfg.showOngoingNotifications
            showQuickFilter = cfg.showQuickFilter
            quickFilterTerm = cfg.quickFilterTerm
            quickFilterIcon = cfg.quickFilterIcon

            tlsClientCertPath = cfg.tlsClientCertPath ?: ""
            tlsClientKeyPath = cfg.tlsClientKeyPath ?: ""

            sortStandardByPriority = cfg.sortStandardByPriority
            pausedSortBehavior = cfg.pausedSortBehavior
            sortTiebreakRecent = cfg.sortTiebreakRecent
            sortPreset = cfg.sortPreset
            sortDays = cfg.sortCutoffDays?.toString() ?: ""
            urgentDays = cfg.urgentDays.toString()
            urgentPrio = cfg.urgentPrio.toString()
            defaultPriority = cfg.defaultPriority.toString()
            startGracePeriodDays = cfg.startGracePeriodDays.toString()
            firstDayOfWeek = cfg.firstDayOfWeek
            showTaskGoalsInSidebar = cfg.showTaskGoalsInSidebar
            defaultDurationGoalMins = cfg.defaultDurationGoalMins.toString()
            sessionsCountAsCompletions = cfg.sessionsCountAsCompletions
        } catch (e: Exception) {
            // Ignore on load
        }
    }

    LaunchedEffect(Unit) { reload() }

    fun saveToDisk() {
        try {
            val cfg = api.getConfig()
            val newCfg = cfg.copy(
                maxDoneRoots = maxDoneRoots.toUIntOrNull() ?: 20u,
                maxDoneSubtasks = maxDoneSubtasks.toUIntOrNull() ?: 5u,
                trashRetention = trashRetention.toUIntOrNull() ?: 14u,
                deleteEventsOnCompletion = deleteEventsOnCompletion,
                strikethroughCompleted = strikethroughCompleted,
                showInlineDescriptions = showInlineDescriptions,
                showOngoingNotifications = showOngoingNotifications,
                showQuickFilter = showQuickFilter,
                quickFilterTerm = quickFilterTerm,
                quickFilterIcon = quickFilterIcon,

                tlsClientCertPath = tlsClientCertPath.takeIf { it.isNotBlank() },
                tlsClientKeyPath = tlsClientKeyPath.takeIf { it.isNotBlank() },

                sortStandardByPriority = sortStandardByPriority,
                pausedSortBehavior = pausedSortBehavior,
                sortTiebreakRecent = sortTiebreakRecent,
                sortPreset = sortPreset,
                sortCutoffDays = sortDays.toUIntOrNull(),
                urgentDays = urgentDays.toUIntOrNull() ?: 1u,
                urgentPrio = urgentPrio.toUByteOrNull() ?: 1u,
                defaultPriority = defaultPriority.toUByteOrNull() ?: 5u,
                startGracePeriodDays = startGracePeriodDays.toUIntOrNull() ?: 1u,
                firstDayOfWeek = firstDayOfWeek,
                showTaskGoalsInSidebar = showTaskGoalsInSidebar,
                defaultDurationGoalMins = defaultDurationGoalMins.toUIntOrNull() ?: 60u,
                sessionsCountAsCompletions = sessionsCountAsCompletions
            )
            api.saveConfig(newCfg)
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
                saveToDisk()
            } else {
                status = context.getString(R.string.tls_client_cert_none)
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
                saveToDisk()
            } else {
                status = context.getString(R.string.tls_client_key_none)
            }
        }
    }

    val handleBack = {
        saveToDisk()
        onBack()
    }

    BackHandler { handleBack() }

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
                        "tiebreak" to "Tie-breaker (within rank/date)",
                        "top" to "Top of list (above unstarted)",
                        "none" to "Off (treat as unstarted)"
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
                        com.trougnouf.cfait.core.MobileFirstDayOfWeek.MONDAY to stringResource(R.string.monday),
                        com.trougnouf.cfait.core.MobileFirstDayOfWeek.SUNDAY to stringResource(R.string.sunday)
                    ),
                    onSelect = { firstDayOfWeek = it; saveToDisk() },
                    modifier = Modifier.width(240.dp)
                )
            }

            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.sorting_preset_label), modifier = Modifier.weight(1f))
                DropdownPicker(
                    label = "",
                    selected = sortPreset,
                    options = listOf(
                        "Urgent > Ongoing > Due Soon" to "Urgent > Ongoing > Due Soon",
                        "Urgent > Due Soon > Ongoing" to "Urgent > Due Soon > Ongoing",
                        "Ongoing > Urgent > Due Soon" to "Ongoing > Urgent > Due Soon"
                    ),
                    onSelect = { sortPreset = it },
                    modifier = Modifier.width(240.dp)
                )
            }
            Text(
                stringResource(R.string.settings_sort_preset_explain),
                fontSize = 12.sp,
                color = androidx.compose.ui.graphics.Color.Gray,
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
                    value = urgentDays,
                    onValueChange = { urgentDays = it },
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.priority_le), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    value = urgentPrio,
                    onValueChange = { urgentPrio = it },
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true
                )
            }
            Text(
                stringResource(R.string.settings_urgent_explain),
                fontSize = 12.sp,
                color = androidx.compose.ui.graphics.Color.Gray,
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
                    value = sortDays,
                    onValueChange = { sortDays = it },
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true
                )
            }
            Text(
                stringResource(R.string.settings_cutoff_explain),
                fontSize = 12.sp,
                color = androidx.compose.ui.graphics.Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 8.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.start_grace_days), modifier = Modifier.weight(1f))
                OutlinedTextField(
                    value = startGracePeriodDays,
                    onValueChange = { startGracePeriodDays = it },
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true
                )
            }
            Text(
                stringResource(R.string.settings_start_grace_explain),
                fontSize = 12.sp,
                color = androidx.compose.ui.graphics.Color.Gray,
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
                    value = defaultPriority,
                    onValueChange = { defaultPriority = it },
                    modifier = Modifier.width(80.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true
                )
            }
            Text(
                stringResource(R.string.settings_default_prio_explain),
                fontSize = 12.sp,
                color = androidx.compose.ui.graphics.Color.Gray,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

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
                value = maxDoneRoots,
                onValueChange = { maxDoneRoots = it },
                label = { Text(stringResource(R.string.max_completed_tasks_root)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                stringResource(R.string.max_completed_tasks_root_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            OutlinedTextField(
                value = maxDoneSubtasks,
                onValueChange = { maxDoneSubtasks = it },
                label = { Text(stringResource(R.string.max_completed_subtasks)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                stringResource(R.string.max_completed_subtasks_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Notifications Section
            Text(
                stringResource(R.string.notifications),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = showOngoingNotifications,
                    onCheckedChange = { showOngoingNotifications = it }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_ongoing_notifications_label))
            }
            Text(
                stringResource(R.string.show_ongoing_notifications_explain),
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
                value = quickFilterTerm,
                onValueChange = { quickFilterTerm = it },
                label = { Text(stringResource(R.string.quick_filter_search_term)) },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                singleLine = true
            )
            OutlinedTextField(
                value = quickFilterIcon,
                onValueChange = { quickFilterIcon = it },
                label = { Text(stringResource(R.string.quick_filter_icon)) },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 16.dp),
                singleLine = true
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
            OutlinedTextField(
                value = trashRetention,
                onValueChange = { trashRetention = it },
                label = { Text(stringResource(R.string.trash_retention_days_label)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
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
                    onCheckedChange = { showTaskGoalsInSidebar = it; saveToDisk() }
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.show_task_goals_in_sidebar))
            }
            OutlinedTextField(
                value = defaultDurationGoalMins,
                onValueChange = { defaultDurationGoalMins = it },
                label = { Text(stringResource(R.string.implicit_goal_duration)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
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
                    onCheckedChange = { sessionsCountAsCompletions = it; saveToDisk() }
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
                            val zipPath = api.createDebugExport()
                            val sourceFile = File(zipPath)
                            val destFile = File(context.cacheDir, "cfait_debug_export.zip")

                            sourceFile.inputStream().use { input ->
                                destFile.outputStream().use { output ->
                                    input.copyTo(output)
                                }
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
                            if (e is kotlinx.coroutines.CancellationException) throw e
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
        }
    }
}
