
// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/TaskRow.kt
// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Composable component for rendering a single task row in the UI.
 */
package com.trougnouf.cfait.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.core.MobileTaskSummary
import com.trougnouf.cfait.ui.StableTaskSummary
import com.trougnouf.cfait.R
import java.time.Instant
import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneId

@Composable
fun TaskCheckbox(
    isDone: Boolean,
    status: String,
    isPaused: Boolean,
    isNote: Boolean,
    calColor: Color,
    onClick: () -> Unit,
) {
    if (isNote) {
        Box(
            modifier = Modifier
                .size(20.dp)
                .background(calColor.copy(alpha = 0.5f), RoundedCornerShape(4.dp))
                .border(1.5.dp, calColor, RoundedCornerShape(4.dp))
        )
        return
    }

    val bgColor =
        when {
            status == "Cancelled" -> Color(0xFF4D3333)
            status == "InProcess" -> Color(0xFF99CC99)
            isPaused -> Color(0xFFFFD54F)
            isDone -> Color(0xFF009900)
            else -> Color.Transparent
        }
    Box(
        modifier =
            Modifier
                .size(
                    20.dp,
                ).background(bgColor, RoundedCornerShape(4.dp))
                .border(1.5.dp, calColor, RoundedCornerShape(4.dp))
                .clickable {
                    onClick()
                },
        contentAlignment = Alignment.Center,
    ) {
        if (status == "Cancelled") {
            Box(Modifier.offset(y = (-1).dp)) { NfIcon(NfIcons.CROSS, 12.sp, Color.White) }
        } else if (status == "InProcess") {
            Box(Modifier.offset(y = (-2).dp)) { NfIcon(NfIcons.PLAY, 10.sp, Color.White) }
        } else if (isPaused) {
            NfIcon(NfIcons.PAUSE, 10.sp, Color.Black)
        } else if (isDone) {
            NfIcon(NfIcons.CHECK, 12.sp, Color.White)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class, ExperimentalFoundationApi::class)
@Composable
fun TaskRow(
    task: StableTaskSummary,
    calColor: Color,
    onToggle: () -> Unit,
    onAction: (String) -> Unit,
    onClick: (String) -> Unit,
    yankedUid: String?,
    enabledCalendarCount: Int,
    isHighlighted: Boolean = false,
    isCollapsed: Boolean = false,
    onToggleCollapse: () -> Unit = {}
) {
    val startPadding = (task.task.depth.toInt() * 12).dp
    var expanded by remember { mutableStateOf(false) }

    // Use the native Android dark mode state detection here if not provided directly
    val isDark = androidx.compose.foundation.isSystemInDarkTheme()

    val textColor = getTaskTextColor(task.task.priority.toInt(), task.task.isDone, isDark)
    val highlightColor = Color(0xFFffe600).copy(alpha = 0.1f)
    val containerColor = if (isHighlighted || expanded) highlightColor else MaterialTheme.colorScheme.surface
    val uriHandler = LocalUriHandler.current

    Card(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(start = 12.dp + startPadding, end = 12.dp, top = 0.5.dp, bottom = 0.5.dp)
                .combinedClickable(
                    onClick = { onClick(task.task.uid) },
                    onLongClick = { expanded = true },
                ),
        colors = CardDefaults.cardColors(containerColor = containerColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp).alpha(if (task.task.isSearchContext) 0.35f else 1.0f), verticalAlignment = Alignment.CenterVertically) {
            TaskCheckbox(
                isDone = task.task.isDone,
                status = task.task.statusString,
                isPaused = task.task.isPaused,
                isNote = task.task.isNote,
                calColor = calColor,
                onClick = onToggle
            )
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                val isTrash = task.task.calendarHref == "local://trash"
                val isStrikethrough = task.task.isDone || isTrash
                val baseStyle = MaterialTheme.typography.bodyMedium.copy(
                    color = textColor,
                    fontWeight = if (task.task.priority > 0.toUByte()) FontWeight.Medium else FontWeight.Normal,
                    lineHeight = 18.sp
                )
                val annotatedSummary = remember(task.task.summary, textColor, isStrikethrough) {
                    com.trougnouf.cfait.ui.parseInlineMarkdown(task.task.summary, textColor, isStrikethrough)
                }
                
                Text(
                    text = annotatedSummary,
                    style = baseStyle,
                )

                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalArrangement = Arrangement.spacedBy(1.dp),
                ) {
                    if (task.task.hasDescription || task.task.hasBlockingTasks) {
                        NfIcon(NfIcons.INFO, size = 10.sp, color = Color.Gray, lineHeight = 10.sp)
                    }

                    if (task.task.hasRelatedTo || task.task.hasRelatedTasks) {
                        val iconName = getRandomRelatedIcon(task.task.uid, "")
                        NfIcon(
                            iconName,
                            size = 10.sp,
                            color = Color.Gray,
                            lineHeight = 10.sp
                        )
                    }

                    if (task.task.isBlocked) NfIcon(
                        NfIcons.BLOCKED,
                        size = 10.sp,
                        color = MaterialTheme.colorScheme.error,
                        lineHeight = 10.sp
                    )

                    if (task.task.hasAlarms) {
                        NfIcon(NfIcons.BELL, size = 10.sp, color = Color(0xFFFF7043), lineHeight = 10.sp)
                    }

                    // --- PRIORITY BOX ---
                    if (task.task.priority.toInt() > 0) {
                        val pColor = getTaskTextColor(task.task.priority.toInt(), task.task.isDone, isDark)
                        Box(modifier = Modifier.border(1.dp, pColor.copy(alpha=0.5f), RoundedCornerShape(4.dp)).padding(horizontal=4.dp, vertical=2.dp)) {
                            Text("!${task.task.priority}", color = pColor, fontSize = 10.sp, lineHeight = 10.sp, fontWeight = FontWeight.Bold)
                        }
                    }

                    // Date Display Logic
                    if (task.task.isDone && task.task.completedDateIso != null) {
                        val (done_icon, doneColor) = if (task.task.statusString == "Completed") {
                            Pair(NfIcons.CALENDAR_CHECK, Color(0xFF66BB6A)) // Greenish
                        } else { // Cancelled
                            Pair(NfIcons.CALENDAR_XMARK, MaterialTheme.colorScheme.error) // Red
                        }

                        NfIcon(done_icon, size = 10.sp, color = doneColor, lineHeight = 10.sp)

                        val dateStr = remember(task.task.completedDateIso) {
                            try {
                                formatIsoToLocal(task.task.completedDateIso!!)
                            } catch (e: Exception) {
                                val safeIso = task.task.completedDateIso ?: ""
                                if (safeIso.length >= 16) safeIso.substring(0, 16).replace("T", " ") else safeIso
                            }
                        }
                        Text(dateStr, fontSize = 10.sp, color = doneColor, lineHeight = 10.sp)

                    } else if (task.task.isFutureStart && task.task.startDateIso != null) {
                        val dimColor = Color(0xFFBDBDBD) // Lighter Gray
                        NfIcon(NfIcons.HOURGLASS_START, size = 10.sp, color = dimColor, lineHeight = 10.sp)

                        val startStr = remember(task.task.startDateIso, task.task.isAlldayStart) {
                            if (task.task.isAlldayStart) {
                                task.task.startDateIso!!.take(10)
                            } else {
                                formatIsoToLocal(task.task.startDateIso!!)
                            }
                        }

                        if (task.task.dueDateIso != null) {
                            val rawDueStr = remember(task.task.dueDateIso, task.task.isAlldayDue) {
                                if (task.task.isAlldayDue) {
                                    task.task.dueDateIso!!.take(10)
                                } else {
                                    formatIsoToLocal(task.task.dueDateIso!!)
                                }
                            }

                            val displayDueStr = if (
                                startStr.length >= 10 &&
                                rawDueStr.length >= 10 &&
                                startStr.substring(0, 10) == rawDueStr.substring(0, 10) &&
                                !task.task.isAlldayDue
                            ) {
                                if (rawDueStr.length > 11) rawDueStr.substring(11) else rawDueStr
                            } else {
                                rawDueStr
                            }

                            if (startStr == rawDueStr) {
                                Text(startStr, fontSize = 10.sp, color = dimColor, lineHeight = 10.sp)
                            } else {
                                Text(
                                    "$startStr - $displayDueStr",
                                    fontSize = 10.sp,
                                    color = dimColor,
                                    lineHeight = 10.sp
                                )
                            }
                            NfIcon(NfIcons.HOURGLASS_END, size = 10.sp, color = dimColor, lineHeight = 10.sp)
                        } else {
                            Text(startStr, fontSize = 10.sp, color = dimColor, lineHeight = 10.sp)
                        }
                    } else if (!task.task.dueDateIso.isNullOrEmpty()) {
                        val isOverdue = remember(task.task.dueDateIso, task.task.isDone, task.task.isAlldayDue) {
                            if (task.task.isDone || task.task.dueDateIso == null) {
                                false
                            } else {
                                try {
                                    val dueInstant = if (task.task.isAlldayDue) {
                                        val localDate = LocalDate.parse(task.task.dueDateIso)
                                        localDate.plusDays(1)
                                            .atStartOfDay(ZoneId.systemDefault())
                                            .toInstant()
                                    } else {
                                        OffsetDateTime.parse(task.task.dueDateIso).toInstant()
                                    }
                                    dueInstant.isBefore(Instant.now())
                                } catch (e: Exception) {
                                    false
                                }
                            }
                        }

                        val dueColor = if (isOverdue) {
                            MaterialTheme.colorScheme.error // Red
                        } else if (task.task.isDueToday) {
                            Color(0xFFFFA726) // Orange
                        } else {
                            Color.Gray
                        };

                        val displayStr = remember(task.task.dueDateIso, task.task.isAlldayDue) {
                            if (task.task.isAlldayDue) {
                                task.task.dueDateIso!!.take(10)
                            } else {
                                formatIsoToLocal(task.task.dueDateIso!!)
                            }
                        }

                        Text(displayStr, fontSize = 10.sp, color = dueColor, lineHeight = 10.sp)
                        NfIcon(NfIcons.HOURGLASS_END, size = 10.sp, color = dueColor, lineHeight = 10.sp)
                    }

                    var liveDurationMins by remember(task.task.timeSpentSeconds, task.task.lastStartedAt) {
                        mutableStateOf((task.task.timeSpentSeconds / 60u).toInt())
                    }

                    if (task.task.lastStartedAt != null) {
                        LaunchedEffect(task.task.lastStartedAt) {
                            while (true) {
                                val now = System.currentTimeMillis() / 1000
                                val start = task.task.lastStartedAt!!
                                val currentSession = if (now > start) now - start else 0
                                val totalSeconds = task.task.timeSpentSeconds.toLong() + currentSession
                                liveDurationMins = (totalSeconds / 60).toInt()
                                kotlinx.coroutines.delay(60000)
                            }
                        }
                    }

                    val pc = task.task.percentComplete
                    val showPc = !task.task.isDone && pc != null && pc > 0u

                    if (liveDurationMins > 0 || task.task.durationMins != null || task.task.lastStartedAt != null || showPc) {
                        val timeLabel = if (task.task.durationMins != null) {
                            val min = task.task.durationMins!!.toInt()
                            val max = task.task.durationMaxMins?.toInt()?.coerceAtLeast(min) ?: min
                            
                            if (liveDurationMins > 0 || task.task.lastStartedAt != null) {
                                val (cStr, maxStr) = formatPairedDuration(liveDurationMins, max)
                                val estDisplay = if (max > min) {
                                    val (_, minStr) = formatPairedDuration(liveDurationMins, min)
                                    "~$minStr-$maxStr"
                                } else {
                                    "~$maxStr"
                                }
                                "$cStr / $estDisplay"
                            } else {
                                if (max > min) {
                                    "~${formatDuration(min.toUInt())}-${formatDuration(max.toUInt())}"
                                } else {
                                    "~${formatDuration(min.toUInt())}"
                                }
                            }
                        } else {
                            if (liveDurationMins > 0 || task.task.lastStartedAt != null) {
                                formatDuration(liveDurationMins.toUInt())
                            } else {
                                ""
                            }
                        }

                        val pcStr = if (showPc) "${pc}%" else ""

                        val label = when {
                            pcStr.isNotEmpty() && timeLabel.isNotEmpty() -> "$pcStr | $timeLabel"
                            pcStr.isNotEmpty() -> pcStr
                            else -> timeLabel
                        }

                        val durColor = if (task.task.lastStartedAt != null) Color(0xFF66BB6A) else Color.Gray

                        Text(label, fontSize = 10.sp, color = durColor, lineHeight = 10.sp)
                    }

                    if (task.task.isRecurring) {
                        val rColor = if (task.task.isRelativeRecurrence) Color(0xFFAB47BC) else Color.Gray
                        NfIcon(NfIcons.REPEAT, size = 10.sp, color = rColor, lineHeight = 10.sp)
                    }

                    if (task.task.geo != null) {
                        IconButton(
                            onClick = { uriHandler.openUri("geo:${task.task.geo}") },
                            modifier = Modifier.size(14.dp).padding(0.dp),
                        ) {
                            NfIcon(
                                NfIcons.MAP_LOCATION_DOT,
                                size = 10.sp,
                                color = Color(0xFF64B5F6),
                                lineHeight = 10.sp
                            )
                        }
                    }

                    // Locations - prefixed with @@ to match user expectation
                    task.task.visibleLocations.forEach { loc ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            val locationColor = Color(0xFFFFB300)
                            Text(
                                "@@",
                                fontSize = 10.sp,
                                color = locationColor,
                                fontWeight = FontWeight.Bold,
                                lineHeight = 10.sp
                            )
                            Text(loc, fontSize = 10.sp, color = locationColor, lineHeight = 10.sp)
                        }
                    }

                    if (task.task.url != null) {
                        IconButton(
                            onClick = { uriHandler.openUri(task.task.url!!) },
                            modifier = Modifier.size(14.dp).padding(0.dp),
                        ) {
                            NfIcon(NfIcons.WEB_CHECK, size = 10.sp, color = Color(0xFF4FC3F7), lineHeight = 10.sp)
                        }
                    }

                    task.task.visibleCategories.forEach { tag ->
                        val bg = getTagColor(tag, isDark)
                        val displayTag = if (tag.contains("=")) tag.substringAfterLast(":") else tag
                        val prefix = if (tag.contains("=")) "" else "#"
                        Text(
                            "$prefix$displayTag",
                            fontSize = 10.sp,
                            color = bg,
                            modifier = Modifier.padding(end = 2.dp),
                            lineHeight = 10.sp
                        )
                    }
                }
            }

            if (yankedUid != null && yankedUid != task.task.uid) {
                IconButton(onClick = { onAction("block") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(NfIcons.BLOCKED, 18.sp, MaterialTheme.colorScheme.secondary)
                }
                IconButton(onClick = { onAction("child") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(NfIcons.CHILD, 18.sp, MaterialTheme.colorScheme.secondary)
                }
                IconButton(onClick = { onAction("related") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(getRandomRelatedIcon(task.task.uid, yankedUid), 18.sp, MaterialTheme.colorScheme.secondary)
                }
            }

            if (task.task.hasVisibleSubtasks || isCollapsed) {
                val trees = listOf(NfIcons.TREE_FA, NfIcons.TREE_FAE, NfIcons.TREE_MD, NfIcons.PALM_TREE, NfIcons.PINE_TREE)
                val hash = kotlin.math.abs(task.task.uid.hashCode())
                val iconChar = if (isCollapsed) {
                    NfIcons.FAMILY_TREE
                } else {
                    trees[hash % 5]
                }
                val iconColor = if (isCollapsed) {
                    MaterialTheme.colorScheme.primary
                } else {
                    // Generate random green shade from hash
                    // G is dominant (0.6-0.9), R and B add variety (0.0-0.2)
                    val r = ((hash shr 16) % 20) / 100f  // 0.0-0.19
                    val g = 0.6f + ((hash shr 8) % 30) / 100f // 0.6-0.89
                    val b = ((hash shr 0) % 20) / 100f   // 0.0-0.19
                    Color(r, g, b, 1f)
                }

                IconButton(onClick = onToggleCollapse, modifier = Modifier.size(24.dp)) {
                    NfIcon(iconChar, 16.sp, iconColor)
                }
            }

            Box {
                IconButton(onClick = { expanded = true }, modifier = Modifier.size(24.dp)) {
                    NfIcon(NfIcons.DOTS_CIRCLE, 16.sp)
                }
                DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.edit)) },
                        onClick = { expanded = false; onClick(task.task.uid) },
                        leadingIcon = { NfIcon(NfIcons.EDIT, 16.sp) })

                    if (!task.task.isNote) {
                        DropdownMenuItem(
                            text = {
                                Text(
                                    if (task.task.statusString == "InProcess") androidx.compose.ui.res.stringResource(R.string.pause)
                                    else if (task.task.isPaused) androidx.compose.ui.res.stringResource(R.string.menu_resume)
                                    else androidx.compose.ui.res.stringResource(R.string.start)
                                )
                            },
                            onClick = { expanded = false; onAction("playpause") },
                            leadingIcon = { NfIcon(if (task.task.statusString == "InProcess") NfIcons.PAUSE else NfIcons.PLAY, 16.sp) }
                        )

                        if (task.task.statusString == "InProcess" || task.task.isPaused) {
                            DropdownMenuItem(
                                text = { Text(androidx.compose.ui.res.stringResource(R.string.stop_reset)) },
                                onClick = { expanded = false; onAction("stop") },
                                leadingIcon = { NfIcon(NfIcons.DEBUG_STOP, 16.sp) })
                        }
                    }

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.increase_priority)) },
                        onClick = { expanded = false; onAction("prio_up") },
                        leadingIcon = { NfIcon(NfIcons.PRIORITY_UP, 16.sp) })

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_decrease_prio)) },
                        onClick = { expanded = false; onAction("prio_down") },
                        leadingIcon = { NfIcon(NfIcons.PRIORITY_DOWN, 16.sp) })

                    if (yankedUid == null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_yank)) },
                            onClick = { expanded = false; onAction("yank") },
                            leadingIcon = { NfIcon(NfIcons.LINK, 16.sp) })
                    }

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.focus_hide_others)) },
                        onClick = { expanded = false; onAction("focus") },
                        leadingIcon = { NfIcon(NfIcons.FOCUS_FIELD, 16.sp) }
                    )

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.action_toggle_pin)) },
                        onClick = { expanded = false; onAction("toggle_pin") },
                        leadingIcon = { NfIcon(NfIcons.THUMB_TACK, 16.sp) }
                    )

                    if (!task.task.isDone && task.task.statusString != "Cancelled" && !task.task.isNote) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.help_metadata_log_time)) },
                            onClick = { expanded = false; onAction("add_session") },
                            leadingIcon = { NfIcon(NfIcons.TIMER_PLUS, 16.sp) }
                        )
                    }

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.create_subtask)) },
                        onClick = { expanded = false; onAction("create_child") },
                        leadingIcon = { NfIcon(NfIcons.CHILD, 16.sp) }
                    )

                    if (task.task.parentUid != null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.promote_remove_parent)) },
                            onClick = { expanded = false; onAction("promote") },
                            leadingIcon = { NfIcon(NfIcons.ELEVATOR_UP, 16.sp) }
                        )
                    }

                    if (task.task.isRecurring && !task.task.isRelativeRecurrence && !task.task.isDone && task.task.statusString != "Cancelled" && !task.task.isNote) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.action_complete_and_shift)) },
                            onClick = { expanded = false; onAction("complete_and_shift") },
                            leadingIcon = { NfIcon(NfIcons.REPEAT, 16.sp) }
                        )
                    }

                    val duplicateLabel = if (task.task.hasSubtasks) androidx.compose.ui.res.stringResource(R.string.duplicate_task)
                    else androidx.compose.ui.res.stringResource(R.string.duplicate_single_task)

                    DropdownMenuItem(
                        text = { Text(duplicateLabel) },
                        onClick = { expanded = false; onAction("duplicate") },
                        leadingIcon = { NfIcon(NfIcons.CLONE, 16.sp) }
                    )

                    if (task.task.hasSubtasks) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.action_complete_tree)) },
                            onClick = { expanded = false; onAction("complete_tree") },
                            leadingIcon = { NfIcon(NfIcons.LIST_CHECK, 16.sp) }
                        )
                    }

                    if (enabledCalendarCount > 1) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_move)) },
                            onClick = { expanded = false; onAction("move") },
                            leadingIcon = { NfIcon(NfIcons.MOVE, 16.sp) })
                    }

                    if (task.task.statusString != "Cancelled") {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.cancel)) },
                            onClick = { expanded = false; onAction("cancel") },
                            leadingIcon = { NfIcon(NfIcons.CROSS, 16.sp) })
                    }

                    if (task.task.geo != null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_location)) },
                            onClick = { expanded = false; uriHandler.openUri("geo:${task.task.geo}") },
                            leadingIcon = { NfIcon(NfIcons.MAP_LOCATION_DOT, 16.sp) })
                    }

                    if (task.task.treeLocationCount.toInt() > 1) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.action_open_locations)) },
                            onClick = { expanded = false; onAction("open_locations_gpx") },
                            leadingIcon = { NfIcon(NfIcons.MAP_MARKER_MULTIPLE, 16.sp) })
                    }

                    if (task.task.url != null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_link)) },
                            onClick = { expanded = false; uriHandler.openUri(task.task.url!!) },
                            leadingIcon = { NfIcon(NfIcons.WEB_CHECK, 16.sp) })
                    }

                    DropdownMenuItem(text = { Text(androidx.compose.ui.res.stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                        onClick = { expanded = false; onAction("delete") },
                        leadingIcon = { NfIcon(NfIcons.DELETE, 16.sp, MaterialTheme.colorScheme.error) })

                    if (task.task.hasSubtasks) {
                        DropdownMenuItem(text = { Text(androidx.compose.ui.res.stringResource(R.string.delete_task_tree), color = MaterialTheme.colorScheme.error) },
                            onClick = { expanded = false; onAction("delete_tree") },
                            leadingIcon = { NfIcon(NfIcons.DELETE, 16.sp, MaterialTheme.colorScheme.error) })
                    }
                }
            }
        }
    }
}

@Composable
fun CompactTagRow(
    name: String,
    count: Int?,
    color: Color,
    isSelected: Boolean,
    onClick: () -> Unit,
    icon: String = NfIcons.TAG,
    onFocus: (() -> Unit)? = null,
    depth: Int = 0,
    hasChildren: Boolean = false,
    isExpanded: Boolean = false,
    onToggleCollapse: (() -> Unit)? = null,
    isTag: Boolean = true
) {
    val bg = if (isSelected) color.copy(alpha = 0.15f) else Color.Transparent
    val indent = (depth * 12).dp

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .height(36.dp)
                .background(bg, RoundedCornerShape(4.dp))
                .clickable { onClick() }
                .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (indent > 0.dp) {
            Spacer(Modifier.width(indent))
        }
        NfIcon(icon, size = 14.sp, color = color)
        Spacer(Modifier.width(12.dp))
        Text(name, fontSize = 14.sp, modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurface)
        if (count != null) Text("$count", fontSize = 12.sp, color = Color.Gray)

        if (hasChildren && onToggleCollapse != null) {
            Spacer(Modifier.width(8.dp))
            val trees = listOf(NfIcons.TREE_FA, NfIcons.TREE_FAE, NfIcons.TREE_MD, NfIcons.PALM_TREE, NfIcons.PINE_TREE)
            val hash = kotlin.math.abs(name.hashCode())
            val iconChar = if (isExpanded) {
                trees[hash % 5]
            } else {
                NfIcons.FAMILY_TREE
            }
            val iconColor = if (isExpanded) {
                val r = ((hash shr 16) % 20) / 100f
                val g = 0.6f + ((hash shr 8) % 30) / 100f
                val b = ((hash shr 0) % 20) / 100f
                Color(r, g, b, 1f)
            } else if (isTag) {
                Color(0xFFB86C00)
            } else {
                Color.Gray
            }

            IconButton(onClick = onToggleCollapse, modifier = Modifier.size(24.dp)) {
                NfIcon(iconChar, 16.sp, iconColor)
            }
        }

        if (onFocus != null) {
            Spacer(Modifier.width(8.dp))
            IconButton(onClick = onFocus, modifier = Modifier.size(24.dp)) {
                NfIcon(NfIcons.ARROW_RIGHT, 14.sp)
            }
        }
    }
}
