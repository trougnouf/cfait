// SPDX-License-Identifier: GPL-3.0-or-later
// Screen for selecting which calendar to import ICS file into.
package com.trougnouf.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.ClickableText
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.withStyle
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileCalendar
import com.trougnouf.cfait.core.MobileRelatedTask
import com.trougnouf.cfait.core.MobileViewData
import com.trougnouf.cfait.R
import com.trougnouf.cfait.ui.CursorContextBanner
import com.trougnouf.cfait.ui.formatDurationHuman
import com.trougnouf.cfait.ui.MarkdownTransformation
import com.trougnouf.cfait.ui.NfIcon
import com.trougnouf.cfait.ui.NfIcons
import com.trougnouf.cfait.ui.triggerBackgroundSync
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.text.SimpleDateFormat
import java.util.*

@Composable
fun IcsImportScreen(
    api: CfaitMobile,
    icsContent: String,
    calendars: List<MobileCalendar>,
    onImportComplete: (String) -> Unit,
    onCancel: () -> Unit
) {
    var selectedCalendar by remember { mutableStateOf<String?>(null) }
    var taskCount by remember { mutableStateOf<Int?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    val context = LocalContext.current

    // Parse ICS content to count tasks
    LaunchedEffect(icsContent) {
        try {
            // Simple count of VTODO entries
            taskCount = icsContent.split("BEGIN:VTODO").size - 1
        } catch (e: Exception) {
            errorMessage = context.getString(R.string.import_failed_to_parse, e.message ?: "")
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
            .padding(bottom = 48.dp), // Extra padding to avoid navigation bar
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        // Header
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                stringResource(R.string.import_action),
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold
            )
        }

        // Info card
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.primaryContainer
            )
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    NfIcon(NfIcons.IMPORT, 20.sp, MaterialTheme.colorScheme.onPrimaryContainer)
                    Text(
                        stringResource(R.string.ics_file_detected),
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }
                val currentTaskCount = taskCount
                if (currentTaskCount != null) {
                    Text(
                        com.trougnouf.cfait.ui.resolvePluralMap(
                            stringResource(R.string.found_tasks_to_import, currentTaskCount),
                            currentTaskCount
                        ),
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }
                if (errorMessage != null) {
                    Text(
                        errorMessage!!,
                        color = MaterialTheme.colorScheme.error
                    )
                }
            }
        }

        // Calendar selection
        Text(
            stringResource(R.string.select_target_collection),
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold
        )

        // Calendar list
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        ) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                items(calendars.filter { !it.isDisabled }) { calendar ->
                    CalendarSelectionItem(
                        calendar = calendar,
                        isSelected = selectedCalendar == calendar.href,
                        onClick = { selectedCalendar = calendar.href }
                    )
                }
            }
        }

        // Action buttons
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedButton(
                onClick = onCancel,
                modifier = Modifier.weight(1f)
            ) {
                Text(androidx.compose.ui.res.stringResource(R.string.cancel))
            }
            Button(
                onClick = {
                    selectedCalendar?.let { href ->
                        onImportComplete(href)
                    }
                },
                modifier = Modifier.weight(1f),
                enabled = selectedCalendar != null && (taskCount ?: 0) > 0
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    NfIcon(NfIcons.IMPORT, 16.sp, MaterialTheme.colorScheme.onPrimary)
                    Text(androidx.compose.ui.res.stringResource(R.string.import_action))
                }
            }
        }
    }
}

@Composable
fun CalendarSelectionItem(
    calendar: MobileCalendar,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(
            containerColor = if (isSelected) {
                MaterialTheme.colorScheme.secondaryContainer
            } else {
                MaterialTheme.colorScheme.surface
            }
        ),
        border = if (isSelected) {
            androidx.compose.foundation.BorderStroke(
                2.dp,
                MaterialTheme.colorScheme.secondary
            )
        } else null
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            RadioButton(
                selected = isSelected,
                onClick = onClick
            )
            Column(
                modifier = Modifier.weight(1f)
            ) {
                Text(
                    text = calendar.name,
                    fontWeight = FontWeight.Medium,
                    fontSize = 16.sp
                )
                Text(
                    text = if (calendar.isLocal) stringResource(R.string.local_collection) else calendar.href,
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun JournalMainView(
    api: CfaitMobile,
    href: String,
    calendars: List<MobileCalendar>,
    onCollectionSelect: (String) -> Unit,
    viewData: com.trougnouf.cfait.core.MobileViewData?,
    journalDateStr: String,
    journalWikiUid: String?,
    journalWikiTitle: String,
    onDateChange: (String) -> Unit,
    onCloseWikiPage: () -> Unit,
    onTaskClick: (String) -> Unit,
    onDataChanged: () -> Unit
) {
    var text by remember { mutableStateOf(androidx.compose.ui.text.input.TextFieldValue("")) }
    var initialText by remember { mutableStateOf("") }
    var titleInput by remember(journalWikiTitle) { mutableStateOf(journalWikiTitle) }
    var initialTitle by remember { mutableStateOf("") }
    var uid by remember { mutableStateOf<String?>(null) }
    var isLoading by remember { mutableStateOf(true) }
    var isSaving by remember { mutableStateOf(false) }

    var undoStack by remember { mutableStateOf(listOf<androidx.compose.ui.text.input.TextFieldValue>()) }
    var redoStack by remember { mutableStateOf(listOf<androidx.compose.ui.text.input.TextFieldValue>()) }
    var showMoveDialog by remember { mutableStateOf(false) }

    val scope = rememberCoroutineScope()
    val isDark = isSystemInDarkTheme()
    val context = LocalContext.current
    
    val enabledCalendarCount = remember(calendars) {
        calendars.count { !it.isDisabled && it.href != "local://trash" && it.href != "local://recovery" }
    }

    LaunchedEffect(href, journalDateStr, journalWikiUid) {
        isLoading = true
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            val targetUid = if (journalWikiUid != null) {
                journalWikiUid
            } else {
                api.getDailyNoteUid(journalDateStr, href)
            }

            val content = if (targetUid != null) {
                api.getTaskTreeMarkdown(targetUid)
            } else {
                ""
            }
            val tfv = androidx.compose.ui.text.input.TextFieldValue(content)
            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
                uid = targetUid
                text = tfv
                initialText = content
                initialTitle = journalWikiTitle
                titleInput = journalWikiTitle
                undoStack = listOf(tfv)
                redoStack = emptyList()
                isLoading = false
            }
        }
    }

    val saveContent = {
        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            isSaving = true
            try {
                val targetUid = if (uid != null) uid!! else {
                    if (journalWikiUid != null) {
                        api.createWikiPage(titleInput, href)
                    } else {
                        api.getOrCreateDailyNote(journalDateStr, href)
                    }
                }
                
                if (journalWikiUid != null) {
                    val t = api.getTaskByUid(targetUid)
                    if (t != null && t.summary != titleInput) {
                        api.updateTaskSmart(targetUid, "is:page $titleInput")
                    }
                }
                
                api.syncTaskTreeFromMarkdown(targetUid, text.text)
                kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
                    uid = targetUid
                    initialText = text.text
                    initialTitle = titleInput
                    onDataChanged()
                }
            } catch (e: Exception) {
                // Ignore
            } finally {
                isSaving = false
            }
        }
    }

    LaunchedEffect(text.text, titleInput) {
        if (isLoading || isSaving) return@LaunchedEffect
        if (text.text == initialText && titleInput == initialTitle) return@LaunchedEffect
        kotlinx.coroutines.delay(1000)
        saveContent()
    }

    if (showMoveDialog) {
        val targetUid = uid ?: journalWikiUid
        if (targetUid != null) {
            val targetCals = remember(calendars) {
                calendars.filter { !it.isDisabled && it.href != "local://trash" && it.href != "local://recovery" }
            }
            AlertDialog(
                onDismissRequest = { showMoveDialog = false },
                title = { Text(stringResource(R.string.move_task_title)) },
                text = {
                    Column {
                        LazyColumn(modifier = Modifier.weight(1f, fill = false)) {
                            items(targetCals) { cal ->
                                TextButton(onClick = {
                                    scope.launch {
                                        try {
                                            api.dispatch(com.trougnouf.cfait.core.AppIntent.MoveTaskTree(targetUid, cal.href))
                                            showMoveDialog = false
                                            onDataChanged()
                                            triggerBackgroundSync(context, api)
                                        } catch (e: Exception) {
                                            if (e is CancellationException) throw e
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
                    }
                },
                confirmButton = {
                    TextButton(onClick = {
                        showMoveDialog = false
                    }) { Text(stringResource(R.string.cancel)) }
                },
            )
        }
    }

    Column(Modifier.fillMaxSize().imePadding()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (journalWikiUid != null) {
                NfIcon(NfIcons.JOURNAL, 20.sp, MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(8.dp))
                OutlinedTextField(
                    value = titleInput,
                    onValueChange = { titleInput = it },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    colors = TextFieldDefaults.colors(
                        focusedContainerColor = Color.Transparent,
                        unfocusedContainerColor = Color.Transparent,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent
                    ),
                    textStyle = androidx.compose.ui.text.TextStyle(fontWeight = FontWeight.Bold, fontSize = 18.sp)
                )
                
                if (enabledCalendarCount > 1) {
                    IconButton(onClick = { showMoveDialog = true }) {
                        NfIcon(NfIcons.MOVE, 20.sp)
                    }
                }
                
                IconButton(onClick = {
                    scope.launch(kotlinx.coroutines.Dispatchers.IO) {
                        try {
                            api.dispatch(com.trougnouf.cfait.core.AppIntent.DeleteTaskTree(journalWikiUid))
                            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
                                onDataChanged()
                                onCloseWikiPage()
                            }
                        } catch (e: Exception) {}
                    }
                }) {
                    NfIcon(NfIcons.DELETE, 20.sp, MaterialTheme.colorScheme.error)
                }
            } else {
                IconButton(onClick = {
                    val sdf = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
                    val c = java.util.Calendar.getInstance()
                    c.time = sdf.parse(journalDateStr) ?: java.util.Date()
                    c.add(java.util.Calendar.DAY_OF_MONTH, -1)
                    onDateChange(sdf.format(c.time))
                }) { NfIcon(NfIcons.ARROW_LEFT) }

                Text(journalDateStr, fontWeight = FontWeight.Bold, fontSize = 18.sp, modifier = Modifier.weight(1f), textAlign = TextAlign.Center)

                IconButton(onClick = {
                    val sdf = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
                    val c = java.util.Calendar.getInstance()
                    c.time = sdf.parse(journalDateStr) ?: java.util.Date()
                    c.add(java.util.Calendar.DAY_OF_MONTH, 1)
                    onDateChange(sdf.format(c.time))
                }) { NfIcon(NfIcons.ARROW_RIGHT) }

                if (uid != null) {
                    if (enabledCalendarCount > 1) {
                        IconButton(onClick = { showMoveDialog = true }) {
                            NfIcon(NfIcons.MOVE, 20.sp)
                        }
                    }
                    IconButton(onClick = {
                        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
                            try {
                                api.dispatch(com.trougnouf.cfait.core.AppIntent.DeleteTaskTree(uid!!))
                                kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
                                    onDataChanged()
                                    text = androidx.compose.ui.text.input.TextFieldValue("")
                                    initialText = ""
                                    uid = null
                                }
                            } catch (e: Exception) {}
                        }
                    }) {
                        NfIcon(NfIcons.DELETE, 20.sp, MaterialTheme.colorScheme.error)
                    }
                }
            }

            if (isSaving || isLoading) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            } else if (text.text != initialText || titleInput != initialTitle) {
                IconButton(onClick = { saveContent() }) {
                    NfIcon(NfIcons.SAVE_AS, 20.sp, MaterialTheme.colorScheme.primary)
                }
            } else {
                IconButton(onClick = {  }) {
                    NfIcon(NfIcons.CHECK, 20.sp, MaterialTheme.colorScheme.primary.copy(alpha = 0.5f))
                }
            }
        }

        if (journalWikiUid == null) {
            val activeCals = calendars.filter { it.isVisible && !it.isDisabled }
            var calHasEntry by remember { mutableStateOf(mapOf<String, Boolean>()) }

            LaunchedEffect(journalDateStr, activeCals) {
                kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                    val map = mutableMapOf<String, Boolean>()
                    for (c in activeCals) {
                        if (api.getDailyNoteUid(journalDateStr, c.href) != null) {
                            map[c.href] = true
                        }
                    }
                    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
                        calHasEntry = map
                    }
                }
            }

            androidx.compose.foundation.lazy.LazyRow(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(activeCals) { cal ->
                    val isSelected = cal.href == href
                    val hasEntry = calHasEntry[cal.href] == true
                    val marker = if (hasEntry) " 📝" else ""
                    val calColorStr = cal.color
                    val calColor = if (calColorStr != null) {
                        com.trougnouf.cfait.ui.parseHexColor(calColorStr)
                    } else {
                        MaterialTheme.colorScheme.primary
                    }

                    FilterChip(
                        selected = isSelected,
                        onClick = { onCollectionSelect(cal.href) },
                        label = { Text(cal.name + marker, fontSize = 13.sp) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = calColor.copy(alpha = 0.2f),
                            selectedLabelColor = calColor,
                            selectedLeadingIconColor = calColor
                        ),
                        border = FilterChipDefaults.filterChipBorder(
                            enabled = true,
                            selected = isSelected,
                            borderColor = calColor.copy(alpha = 0.5f),
                            selectedBorderColor = calColor
                        )
                    )
                }
            }
        }

        HorizontalDivider()

        if (isLoading) {
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        } else {
            OutlinedTextField(
                value = text,
                onValueChange = { newValue ->
                    var finalValue = newValue
                    if (newValue.text.length > text.text.length && 
                        newValue.selection.start == text.selection.start + 1 &&
                        newValue.text[text.selection.start] == '\n'
                    ) {
                        val cursor = text.selection.start
                        val lineStart = text.text.lastIndexOf('\n', cursor - 1).let { if (it == -1) 0 else it + 1 }
                        val prevLine = text.text.substring(lineStart, cursor)
                        val prefix = api.extractListPrefix(prevLine)
                        
                        if (prefix.isNotEmpty()) {
                            if (prevLine.trim() == prefix.trim()) {
                                val before = text.text.substring(0, lineStart)
                                val after = newValue.text.substring(newValue.selection.start)
                                val newText = before + after
                                finalValue = androidx.compose.ui.text.input.TextFieldValue(text = newText, selection = androidx.compose.ui.text.TextRange(lineStart))
                            } else {
                                val before = newValue.text.substring(0, newValue.selection.start)
                                val after = newValue.text.substring(newValue.selection.start)
                                val newText = before + prefix + after
                                finalValue = androidx.compose.ui.text.input.TextFieldValue(text = newText, selection = androidx.compose.ui.text.TextRange(newValue.selection.start + prefix.length))
                            }
                        }
                    }

                    if (finalValue.text != text.text) {
                        undoStack = (undoStack + finalValue).takeLast(50)
                        redoStack = emptyList()
                    }
                    text = finalValue 
                },
                modifier = Modifier.weight(1f).fillMaxWidth().padding(8.dp),
                placeholder = { Text(stringResource(R.string.notes_placeholder)) },
                visualTransformation = remember(isDark) { MarkdownTransformation(isDark, api) },
                textStyle = androidx.compose.ui.text.TextStyle(fontSize = 15.sp),
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = Color.Transparent,
                    unfocusedContainerColor = Color.Transparent,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent
                )
            )
            CursorContextBanner(api, text) { text = it }
        }

        if (journalWikiUid == null && viewData != null) {
            val ctxData = viewData.journalContext
            if (ctxData.totalTrackedMins > 0u || ctxData.dueTasks.isNotEmpty() || ctxData.startedTasks.isNotEmpty() || ctxData.ongoingTasks.isNotEmpty() || ctxData.completedTasks.isNotEmpty()) {
                HorizontalDivider()
                androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxWidth().heightIn(max = 200.dp).padding(8.dp)) {
                    item {
                        Text(stringResource(R.string.journal_activity), fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 4.dp))
                    }
                    if (ctxData.totalTrackedMins > 0u) {
                        item {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                NfIcon(NfIcons.TIMER_SETTINGS, 12.sp, Color(0xFF4CAF50))
                                Spacer(Modifier.width(4.dp))
                                Text(stringResource(R.string.journal_time_tracked) + ": " + formatDurationHuman(ctxData.totalTrackedMins.toLong()), fontSize = 13.sp, color = Color(0xFF4CAF50))
                            }
                        }
                    }

                    val renderList = @Composable { titleRes: Int, icon: String, iconColor: Color, items: List<com.trougnouf.cfait.core.MobileRelatedTask> ->
                        if (items.isNotEmpty()) {
                            Column(Modifier.padding(top = 4.dp)) {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    NfIcon(icon, 10.sp, iconColor)
                                    Spacer(Modifier.width(4.dp))
                                    Text(stringResource(titleRes) + ":", fontSize = 12.sp, color = Color.Gray)
                                }
                                val styledText = androidx.compose.ui.text.buildAnnotatedString {
                                    items.forEachIndexed { index, task ->
                                        pushStringAnnotation("UID", task.uid)
                                        withStyle(androidx.compose.ui.text.SpanStyle(color = Color(0xFF2196F3))) {
                                            append(task.summary)
                                        }
                                        pop()
                                        if (index < items.size - 1) {
                                            withStyle(androidx.compose.ui.text.SpanStyle(color = Color.Gray)) {
                                                append(", ")
                                            }
                                        }
                                    }
                                }
                                androidx.compose.foundation.text.ClickableText(
                                    text = styledText,
                                    style = androidx.compose.ui.text.TextStyle(fontSize = 13.sp),
                                    onClick = { offset ->
                                        styledText.getStringAnnotations("UID", offset, offset).firstOrNull()?.let {
                                            onTaskClick(it.item)
                                        }
                                    }
                                )
                            }
                        }
                    }

                    item { renderList(R.string.journal_worked_on_today, NfIcons.PLAY, Color(0xFF4CAF50), ctxData.startedTasks + ctxData.ongoingTasks) }
                    item { renderList(R.string.journal_completed_today, NfIcons.CHECK, Color(0xFF4CAF50), ctxData.completedTasks) }
                    item { renderList(R.string.journal_due_today, NfIcons.CALENDAR, Color(0xFFFFA000), ctxData.dueTasks) }
                }
            }
        }
    }
}
