// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/TaskDetailScreen.kt
// Compose UI screen for editing task details.
package com.trougnouf.cfait.ui

import androidx.activity.compose.BackHandler
import android.Manifest
import android.content.pm.PackageManager
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileCalendar
import com.trougnouf.cfait.core.MobileTask
import com.trougnouf.cfait.core.MobileRelatedTask
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaskDetailScreen(
    api: CfaitMobile,
    uid: String,
    calendars: List<MobileCalendar>,
    onBack: () -> Unit,
    onSave: (String, String) -> Unit,
    onNavigate: (String) -> Unit,
) {
    var task by remember { mutableStateOf<MobileTask?>(null) }
    val scope = rememberCoroutineScope()
    var smartInput by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var showMoveDialog by remember { mutableStateOf(false) }
    val isDark = isSystemInDarkTheme()
    val uriHandler = LocalUriHandler.current
    val context = LocalContext.current

    // --- Geolocation State ---
    var pendingGeoInput by remember { mutableStateOf<String?>(null) }
    var pendingGeoDesc by remember { mutableStateOf<String?>(null) }

    val locationPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        val granted = permissions.entries.any { it.value }
        scope.launch {
            val input = pendingGeoInput ?: return@launch
            val desc = pendingGeoDesc ?: ""
            pendingGeoInput = null
            pendingGeoDesc = null

            if (granted) {
                val loc = fetchCurrentLocation(context)
                if (loc != null) {
                    val resolved = input.replace(
                        Regex("geo:here", RegexOption.IGNORE_CASE),
                        "geo:${loc.latitude},${loc.longitude}"
                    )
                    onSave(resolved, desc)
                } else {
                    Toast.makeText(
                        context,
                        context.getString(R.string.could_not_determine_location),
                        Toast.LENGTH_SHORT
                    ).show()
                    onSave(input, desc)
                }
            } else {
                onSave(input, desc)
            }
        }
    }

    val enabledCalendarCount =
        remember(calendars) {
            calendars.count { !it.isDisabled }
        }

    fun reload() {
        scope.launch {
            // Use direct lookup instead of searching in the filtered view list.
            // This ensures completed/hidden tasks can still be opened and edited.
            task = api.getTaskByUid(uid)
            task?.let {
                smartInput = it.smartString
                description = it.description
            }
        }
    }

    LaunchedEffect(uid) { reload() }

    BackHandler {
        onBack()
    }

    fun handleSaveWithGeo(input: String, desc: String) {
        if (input.contains("geo:here", ignoreCase = true)) {
            val hasFine = ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.ACCESS_FINE_LOCATION
            ) == PackageManager.PERMISSION_GRANTED
            val hasCoarse = ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.ACCESS_COARSE_LOCATION
            ) == PackageManager.PERMISSION_GRANTED

            if (hasFine || hasCoarse) {
                scope.launch {
                    val loc = fetchCurrentLocation(context)
                    if (loc != null) {
                        onSave(
                            input.replace(
                                Regex("geo:here", RegexOption.IGNORE_CASE),
                                "geo:${loc.latitude},${loc.longitude}"
                            ), desc
                        )
                    } else {
                        Toast.makeText(
                            context,
                            context.getString(R.string.could_not_determine_location),
                            Toast.LENGTH_SHORT
                        ).show()
                        onSave(input, desc)
                    }
                }
            } else {
                pendingGeoInput = input
                pendingGeoDesc = desc
                locationPermissionLauncher.launch(
                    arrayOf(
                        Manifest.permission.ACCESS_FINE_LOCATION,
                        Manifest.permission.ACCESS_COARSE_LOCATION
                    )
                )
            }
        } else {
            onSave(input, desc)
        }
    }

    if (task == null) {
        Box(Modifier.fillMaxSize()) { CircularProgressIndicator(Modifier.align(Alignment.Center)) }
        return
    }

    if (showMoveDialog) {
        val targetCals =
            remember(calendars) {
                calendars.filter { it.href != task!!.calendarHref && !it.isDisabled }
            }
        AlertDialog(
            onDismissRequest = { showMoveDialog = false },
            title = { Text(stringResource(R.string.move_task_title)) },
            text = {
                LazyColumn {
                    items(targetCals) { cal ->
                        TextButton(onClick = {
                            scope.launch {
                                try {
                                    api.moveTask(uid, cal.href)
                                    showMoveDialog = false
                                    onBack()
                                } catch (e: Exception) {
                                    android.widget.Toast.makeText(
                                        context,
                                        "Error: ${e.message}",
                                        android.widget.Toast.LENGTH_SHORT
                                    ).show()
                                }
                            }
                        }, modifier = Modifier.fillMaxWidth()) { Text(cal.name) }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    showMoveDialog = false
                }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.edit_task_title)) },
                navigationIcon = { IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) } },
                actions = {
                    if (task!!.geo != null) {
                        IconButton(onClick = { uriHandler.openUri("geo:${task!!.geo}") }) {
                            NfIcon(NfIcons.MAP_LOCATION_DOT, 20.sp)
                        }
                    }
                    if (task!!.url != null) {
                        IconButton(onClick = { uriHandler.openUri(task!!.url!!) }) {
                            NfIcon(NfIcons.WEB_CHECK, 20.sp)
                        }
                    }
                    if (enabledCalendarCount > 1) {
                        TextButton(onClick = {
                            showMoveDialog = true
                        }) { Text(stringResource(R.string.menu_move)) }
                    }
                    TextButton(
                        onClick = {
                            // Optimistic Save:
                            // We delegate the actual async work to the parent (MainActivity)
                            // so we can leave this screen immediately without killing the save process.
                            handleSaveWithGeo(smartInput, description)
                        },
                    ) { Text(stringResource(R.string.save)) }
                },
            )
        },
    ) { p ->
        val scrollState = rememberScrollState()
        Column(
            modifier = Modifier
                .padding(p)
                .imePadding()
                .padding(start = 16.dp, top = 16.dp, end = 16.dp)
                .verticalScroll(scrollState)
        ) {
            OutlinedTextField(
                value = smartInput,
                onValueChange = { smartInput = it.replace("\n", "") }, // Manually block newlines
                label = { Text(stringResource(R.string.task_smart_syntax_label)) },
                modifier = Modifier.fillMaxWidth(),
                visualTransformation = remember(isDark) { SmartSyntaxTransformation(api, isDark) },
                // Removed singleLine = true to avoid cursor handle positioning issues on high DPI tablets
                maxLines = 5,
                keyboardOptions = KeyboardOptions.Default.copy(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = {
                    handleSaveWithGeo(smartInput, description)
                }),
            )
            Text(
                stringResource(R.string.help_syntax_short),
                style = MaterialTheme.typography.bodySmall,
                color = androidx.compose.ui.graphics.Color.Gray,
                modifier = Modifier.padding(start = 4.dp, bottom = 16.dp),
            )

            if (task!!.blockedByNames.isNotEmpty()) {
                Text(
                    stringResource(R.string.blocked_by),
                    color = MaterialTheme.colorScheme.error,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )

                val blockedPairs = task!!.blockedByNames.zip(task!!.blockedByUids)

                blockedPairs.forEach { (name, blockerUid) ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(vertical = 2.dp)
                    ) {
                        // DELETE BUTTON
                        IconButton(
                            onClick = {
                                scope.launch {
                                    try {
                                        api.removeDependency(task!!.uid, blockerUid)
                                        reload()
                                    } catch (e: Exception) {
                                        android.widget.Toast.makeText(
                                            context,
                                            "Error: ${e.message}",
                                            android.widget.Toast.LENGTH_SHORT
                                        ).show()
                                    }
                                }
                            },
                            modifier = Modifier.size(24.dp)
                        ) {
                            NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.error)
                        }

                        Spacer(Modifier.width(8.dp))

                        // NAVIGATION AREA
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .clickable { onNavigate(blockerUid) }
                                .padding(4.dp)
                        ) {
                            NfIcon(NfIcons.BLOCKED, 12.sp, androidx.compose.ui.graphics.Color.Gray)
                            Spacer(Modifier.width(4.dp))
                            Text(name, fontSize = 14.sp)
                        }
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
            }

            // Blocking (Successors) - tasks that are blocked BY this task
            if (task!!.blockingNames.isNotEmpty()) {
                Text(
                    stringResource(R.string.blocking_label),
                    color = MaterialTheme.colorScheme.tertiary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )

                val blockingPairs = task!!.blockingNames.zip(task!!.blockingUids)

                blockingPairs.forEach { (name, blockedUid) ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(vertical = 2.dp)
                    ) {
                        // UNLINK BUTTON (remove THIS task from the OTHER task's dependency list)
                        IconButton(
                            onClick = {
                                scope.launch {
                                    try {
                                        // To unblock, remove this task.uid from the blocked task's dependencies
                                        api.removeDependency(blockedUid, task!!.uid)
                                        reload()
                                    } catch (e: Exception) {
                                        android.widget.Toast.makeText(
                                            context,
                                            "Error: ${e.message}",
                                            android.widget.Toast.LENGTH_SHORT
                                        ).show()
                                    }
                                }
                            },
                            modifier = Modifier.size(24.dp)
                        ) {
                            NfIcon(NfIcons.UNLINK, 12.sp, MaterialTheme.colorScheme.tertiary)
                        }

                        Spacer(Modifier.width(8.dp))

                        // NAVIGATION AREA
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .clickable { onNavigate(blockedUid) }
                                .padding(4.dp)
                        ) {
                            // Use Down Arrow to indicate successor flow
                            NfIcon(NfIcons.HAND_STOP, 12.sp, androidx.compose.ui.graphics.Color.Gray)
                            Spacer(Modifier.width(4.dp))
                            Text(name, fontSize = 14.sp)
                        }
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
            }

            if (task!!.relatedToNames.isNotEmpty()) {
                Text(
                    stringResource(R.string.related_to_label),
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )

                val relatedPairs = task!!.relatedToNames.zip(task!!.relatedToUids)

                relatedPairs.forEach { (name, relatedUid) ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(vertical = 2.dp)
                    ) {
                        // DELETE BUTTON
                        IconButton(
                            onClick = {
                                scope.launch {
                                    try {
                                        api.removeRelatedTo(task!!.uid, relatedUid)
                                        reload()
                                    } catch (e: Exception) {
                                        android.widget.Toast.makeText(
                                            context,
                                            "Error: ${e.message}",
                                            android.widget.Toast.LENGTH_SHORT
                                        ).show()
                                    }
                                }
                            },
                            modifier = Modifier.size(24.dp)
                        ) {
                            NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.error)
                        }

                        Spacer(Modifier.width(8.dp))

                        // NAVIGATION AREA
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .clickable { onNavigate(relatedUid) }
                                .padding(4.dp)
                        ) {
                            NfIcon(
                                getRandomRelatedIcon(task!!.uid, relatedUid),
                                12.sp,
                                androidx.compose.ui.graphics.Color.Gray
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(name, fontSize = 14.sp)
                        }
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
            }

            // --- Work Sessions Block ---
            var showAddSession by remember { mutableStateOf(false) }
            var sessionInput by remember { mutableStateOf("") }
            var showAllSessions by remember { mutableStateOf(false) }

            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                val totalMins = task!!.sessions.sumOf { (it.endMs - it.startMs) / 60000 }
                Text(
                    stringResource(R.string.time_tracked_duration, totalMins / 60, totalMins % 60),
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )
                if (!showAddSession) {
                    IconButton(onClick = { showAddSession = true }, modifier = Modifier.size(24.dp)) {
                        NfIcon(NfIcons.TIMER_PLUS, 16.sp, MaterialTheme.colorScheme.primary)
                    }
                }
            }

            if (showAddSession) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)
                ) {
                    OutlinedTextField(
                        value = sessionInput,
                        onValueChange = { sessionInput = it },
                        placeholder = { Text(stringResource(R.string.eg) + " 30m, yesterday 2h") },
                        modifier = Modifier.weight(1f),
                        singleLine = true
                    )
                    IconButton(
                        onClick = {
                            if (sessionInput.isNotBlank()) {
                                scope.launch {
                                    try {
                                        api.addSession(uid, sessionInput)
                                        sessionInput = ""
                                        showAddSession = false
                                        reload()
                                    } catch (e: Exception) {
                                        val msg = e.message ?: ""
                                        if (msg.contains("Invalid time format") || msg.contains("Task not found")) {
                                            android.widget.Toast.makeText(
                                                context,
                                                "Format error: $msg",
                                                android.widget.Toast.LENGTH_SHORT
                                            ).show()
                                        } else {
                                            // The time session was successfully saved locally, but the
                                            // subsequent network sync encountered an error.
                                            // We gracefully swallow it to keep the UX seamless.
                                            android.util.Log.e("CfaitUI", "Sync delayed after session: $msg")
                                            sessionInput = ""
                                            showAddSession = false
                                            reload()
                                        }
                                    }
                                }
                            }
                        }
                    ) { NfIcon(NfIcons.CHECK, 16.sp, MaterialTheme.colorScheme.primary) }
                    IconButton(onClick = { showAddSession = false }) {
                        NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.error)
                    }
                }
            }

            val visibleSessions = if (showAllSessions) {
                task!!.sessions.reversed()
            } else {
                task!!.sessions.reversed().take(3)
            }

            visibleSessions.forEachIndexed { revIdx, session ->
                // Convert reversed index back to absolute index for deletion
                val absoluteIdx = task!!.sessions.size - 1 - revIdx

                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp)
                ) {
                    val startDt = java.time.Instant.ofEpochMilli(session.startMs)
                        .atZone(java.time.ZoneId.systemDefault())
                    val endDt = java.time.Instant.ofEpochMilli(session.endMs)
                        .atZone(java.time.ZoneId.systemDefault())
                    val formatterDate = java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd")
                    val formatterTime = java.time.format.DateTimeFormatter.ofPattern("HH:mm")
                    val durMins = (session.endMs - session.startMs) / 60000

                    val dateStr = startDt.format(formatterDate)
                    val timeRange = "${startDt.format(formatterTime)}-${endDt.format(formatterTime)}"

                    Text(
                        "$dateStr $timeRange",
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.8f)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        "(${durMins}m)",
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
                        modifier = Modifier.weight(1f)
                    )

                    IconButton(
                        onClick = {
                            scope.launch {
                                try {
                                    api.deleteSession(uid, absoluteIdx.toUInt())
                                    reload()
                                } catch (e: Exception) {
                                    android.widget.Toast.makeText(
                                        context,
                                        "Error deleting session: ${e.message}",
                                        android.widget.Toast.LENGTH_SHORT
                                    ).show()
                                }
                            }
                        },
                        modifier = Modifier.size(24.dp)
                    ) {
                        NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.error)
                    }
                }
            }

            if (task!!.sessions.size > 3) {
                TextButton(
                    onClick = { showAllSessions = !showAllSessions },
                    contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.height(24.dp)
                ) {
                    Text(
                        if (showAllSessions) stringResource(R.string.show_less) else stringResource(
                            R.string.show_older_sessions,
                            task!!.sessions.size - 3
                        ),
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.secondary
                    )
                }
            }
            HorizontalDivider(Modifier.padding(vertical = 8.dp))

            var incomingRelated by remember { mutableStateOf<List<MobileRelatedTask>>(emptyList()) }

            LaunchedEffect(task) {
                incomingRelated = if (task != null) {
                    api.getTasksRelatedTo(task!!.uid)
                } else {
                    emptyList()
                }
            }
            if (incomingRelated.isNotEmpty()) {
                Text(
                    stringResource(R.string.related_from_label),
                    color = MaterialTheme.colorScheme.secondary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )

                incomingRelated.forEach { relatedTask ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(vertical = 2.dp)
                    ) {
                        // DELETE BUTTON
                        IconButton(
                            onClick = {
                                scope.launch {
                                    try {
                                        api.removeRelatedTo(relatedTask.uid, task!!.uid)
                                        reload()
                                    } catch (e: Exception) {
                                        android.widget.Toast.makeText(
                                            context,
                                            "Error: ${e.message}",
                                            android.widget.Toast.LENGTH_SHORT
                                        ).show()
                                    }
                                }
                            },
                            modifier = Modifier.size(24.dp)
                        ) {
                            NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.error)
                        }

                        Spacer(Modifier.width(8.dp))

                        // NAVIGATION AREA
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .clickable { onNavigate(relatedTask.uid) }
                                .padding(4.dp)
                        ) {
                            NfIcon(
                                getRandomRelatedIcon(task!!.uid, relatedTask.uid),
                                12.sp,
                                androidx.compose.ui.graphics.Color.Gray
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(relatedTask.summary, fontSize = 14.sp)
                        }
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
            }

            OutlinedTextField(
                value = description,
                onValueChange = { description = it },
                label = { Text(stringResource(R.string.description_label)) },
                modifier = Modifier.fillMaxWidth().heightIn(min = 150.dp),
                textStyle = TextStyle(textAlign = androidx.compose.ui.text.style.TextAlign.Start),
            )

            Spacer(Modifier.height(24.dp))
        }
    }
}
