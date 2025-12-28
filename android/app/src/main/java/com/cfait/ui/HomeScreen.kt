// File: android/app/src/main/java/com/cfait/ui/HomeScreen.kt
package com.cfait.ui

import android.content.ClipData
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.R
import com.cfait.core.*
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    api: CfaitMobile,
    calendars: List<MobileCalendar>,
    tags: List<MobileTag>,
    locations: List<MobileLocation>,
    defaultCalHref: String?,
    isLoading: Boolean,
    hasUnsynced: Boolean,
    autoScrollUid: String? = null,
    refreshTick: Long, // FIX: Added parameter
    onGlobalRefresh: () -> Unit,
    onSettings: () -> Unit,
    onTaskClick: (String) -> Unit,
    onDataChanged: () -> Unit,
) {
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var sidebarTab by rememberSaveable { mutableIntStateOf(0) }
    var isManualSyncing by remember { mutableStateOf(false) }
    var activeOpCount by remember { mutableIntStateOf(0) }
    var lastSyncFailed by remember { mutableStateOf(false) }
    var localHasUnsynced by remember { mutableStateOf(hasUnsynced) }
    var isPullRefreshing by remember { mutableStateOf(false) }
    var filterLocation by rememberSaveable { mutableStateOf<String?>(null) }
    var taskToMove by remember { mutableStateOf<MobileTask?>(null) }

    val locationTabIcon = rememberSaveable {
        val icons = listOf(
            NfIcons.LOCATION, NfIcons.EARTH_ASIA, NfIcons.EARTH_AMERICAS,
            NfIcons.EARTH_AFRICA, NfIcons.EARTH_GENERIC, NfIcons.PLANET,
            NfIcons.GALAXY, NfIcons.ISLAND, NfIcons.COMPASS,
            NfIcons.MOUNTAINS, NfIcons.GLOBE, NfIcons.GLOBEMODEL,
        )
        icons.random()
    }

    val enabledCalendarCount = remember(calendars) {
        calendars.count { !it.isDisabled }
    }

    LaunchedEffect(hasUnsynced) { localHasUnsynced = hasUnsynced }

    fun checkSyncStatus() {
        scope.launch {
            try {
                localHasUnsynced = api.hasUnsyncedChanges()
            } catch (_: Exception) {
            }
        }
    }

    var hasSetDefaultTab by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(calendars) {
        if (!hasSetDefaultTab && calendars.isNotEmpty()) {
            val hasRemote = calendars.any { !it.isLocal }
            if (!hasRemote) {
                sidebarTab = 1
            }
            hasSetDefaultTab = true
        }
    }

    val listState = rememberLazyListState()
    var tasks by remember { mutableStateOf<List<MobileTask>>(emptyList()) }
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var filterTag by rememberSaveable { mutableStateOf<String?>(null) }
    var isSearchActive by rememberSaveable { mutableStateOf(false) }
    val searchFocusRequester = remember { FocusRequester() }
    var newTaskText by remember { mutableStateOf("") }
    var showExportDialog by remember { mutableStateOf(false) }
    var yankedUid by remember { mutableStateOf<String?>(null) }
    val yankedTask = remember(tasks, yankedUid) { tasks.find { it.uid == yankedUid } }
    var creatingChildUid by remember { mutableStateOf<String?>(null) }
    val creatingChildTask = remember(tasks, creatingChildUid) { tasks.find { it.uid == creatingChildUid } }
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val isDark = isSystemInDarkTheme()
    val calColorMap = remember(calendars) {
        calendars.associate { it.href to (it.color?.let { hex -> parseHexColor(hex) } ?: Color.Gray) }
    }

    BackHandler(enabled = drawerState.isOpen) { scope.launch { drawerState.close() } }

    fun updateTaskList() {
        scope.launch {
            try {
                tasks = api.getViewTasks(filterTag, filterLocation, searchQuery)
            } catch (_: Exception) {
            }
        }
    }

    // Add refreshTick to LaunchedEffect keys to force task reload
    LaunchedEffect(searchQuery, filterTag, filterLocation, isLoading, calendars, tags, locations, refreshTick) {
        updateTaskList()
    }

    // ... rest of the file unchanged ...
    val handleRefresh = {
        scope.launch {
            isManualSyncing = true
            try {
                api.sync()
                lastSyncFailed = false
                onDataChanged()
                updateTaskList()
            } catch (e: Exception) {
                lastSyncFailed = true
                Toast.makeText(context, "Sync issue: ${e.message}", Toast.LENGTH_SHORT).show()
                api.loadFromCache()
                updateTaskList()
            } finally {
                checkSyncStatus()
                isManualSyncing = false
            }
        }
    }

    val handlePullRefresh = {
        scope.launch {
            isPullRefreshing = true
            try {
                api.sync()
                lastSyncFailed = false
                onDataChanged()
                updateTaskList()
            } catch (e: Exception) {
                lastSyncFailed = true
                Toast.makeText(context, "Sync issue: ${e.message}", Toast.LENGTH_SHORT).show()
                api.loadFromCache()
                updateTaskList()
            } finally {
                checkSyncStatus()
                isPullRefreshing = false
            }
        }
    }

    LaunchedEffect(autoScrollUid, tasks) {
        if (autoScrollUid != null) {
            val index = tasks.indexOfFirst { it.uid == autoScrollUid }
            if (index >= 0) listState.animateScrollToItem(index)
        }
    }

    fun toggleTask(task: MobileTask) {
        val newIsDone = !task.isDone
        val newStatus = if (newIsDone) "Completed" else "NeedsAction"
        tasks =
            tasks.map {
                if (it.uid == task.uid) it.copy(isDone = newIsDone, statusString = newStatus, isPaused = false) else it
            }
        scope.launch {
            activeOpCount++
            try {
                api.toggleTask(task.uid)
                updateTaskList()
                checkSyncStatus()
                onDataChanged()
                lastSyncFailed = false
            } catch (_: Exception) {
                lastSyncFailed = true
                updateTaskList()
                checkSyncStatus()
            } finally {
                activeOpCount--
            }
        }
    }

    fun addTask(txt: String) {
        val text = txt.trim()
        val isAliasDef = text.contains(":=")

        if (text.startsWith("#") && !text.contains(" ") && !isAliasDef) {
            val tag = text.removePrefix("#")
            filterTag = tag
            sidebarTab = 1
            newTaskText = ""
            updateTaskList()
        } else if ((text.startsWith("@@") || text.startsWith("loc:")) && !text.contains(" ") && !isAliasDef) {
            val loc =
                if (text.startsWith("@@")) {
                    text.removePrefix("@@")
                } else {
                    text.removePrefix("loc:")
                }
            val cleanLoc = loc.replace("\"", "")

            filterLocation = cleanLoc
            sidebarTab = 2
            newTaskText = ""
            updateTaskList()
        } else {
            newTaskText = ""
            scope.launch {
                activeOpCount++
                try {
                    val newUid = api.addTaskSmart(text)

                    if (creatingChildUid != null) {
                        api.setParent(newUid, creatingChildUid!!)
                        creatingChildUid = null
                    }

                    onDataChanged()
                    lastSyncFailed = false
                    try {
                        val newTasks = api.getViewTasks(filterTag, filterLocation, searchQuery)
                        tasks = newTasks
                        val index = newTasks.indexOfFirst { it.uid == newUid }
                        if (index >= 0) listState.animateScrollToItem(index)
                    } catch (_: Exception) {
                    }
                } catch (_: Exception) {
                    lastSyncFailed = true
                } finally {
                    checkSyncStatus()
                    activeOpCount--
                }
            }
        }
    }

    fun onTaskAction(
        action: String,
        task: MobileTask,
    ) {
        if (action == "move") {
            taskToMove = task
            return
        }

        if (action == "create_child") {
            creatingChildUid = task.uid
            yankedUid = null
            return
        }

        val updatedList =
            tasks
                .map { t ->
                    if (t.uid == task.uid) {
                        when (action) {
                            "delete" -> {
                                null
                            }

                            "cancel" -> {
                                t.copy(statusString = "Cancelled", isDone = false)
                            }

                            "playpause" -> {
                                if (t.statusString == "InProcess") {
                                    t.copy(statusString = "NeedsAction", isPaused = true)
                                } else {
                                    t.copy(statusString = "InProcess", isPaused = false)
                                }
                            }

                            "stop" -> {
                                t.copy(statusString = "NeedsAction", isPaused = false)
                            }

                            "prio_up" -> {
                                var p = t.priority.toInt()
                                if (p == 0) p = 5
                                if (p > 1) p -= 1
                                t.copy(priority = p.toUByte())
                            }

                            "prio_down" -> {
                                var p = t.priority.toInt()
                                if (p == 0) p = 5
                                if (p < 9) p += 1
                                t.copy(priority = p.toUByte())
                            }

                            else -> {
                                t
                            }
                        }
                    } else {
                        t
                    }
                }.filterNotNull()
        tasks = updatedList

        scope.launch {
            activeOpCount++
            try {
                when (action) {
                    "delete" -> {
                        api.deleteTask(task.uid)
                    }

                    "cancel" -> {
                        api.setStatusCancelled(task.uid)
                    }

                    "playpause" -> {
                        if (task.statusString == "InProcess") api.pauseTask(task.uid) else api.startTask(task.uid)
                    }

                    "stop" -> {
                        api.stopTask(task.uid)
                    }

                    "prio_up" -> {
                        api.changePriority(task.uid, 1)
                    }

                    "prio_down" -> {
                        api.changePriority(task.uid, -1)
                    }

                    "yank" -> {
                        yankedUid = task.uid
                        val clipData = ClipData.newPlainText("task_uid", task.uid)
                        clipboard.setClipEntry(ClipEntry(clipData))
                    }

                    "block" -> {
                        if (yankedUid != null) {
                            api.addDependency(task.uid, yankedUid!!)
                            yankedUid = null
                        }
                    }

                    "child" -> {
                        if (yankedUid != null) {
                            api.setParent(task.uid, yankedUid!!)
                            yankedUid = null
                        }
                    }
                }
                updateTaskList()
                onDataChanged()
                lastSyncFailed = false
            } catch (_: Exception) {
                lastSyncFailed = true
                updateTaskList()
            } finally {
                checkSyncStatus()
                activeOpCount--
            }
        }
    }

    val remoteCals = remember(calendars) { calendars.filter { !it.isLocal && !it.isDisabled } }

    if (taskToMove != null) {
        val targetCals =
            remember(calendars) {
                calendars.filter { it.href != taskToMove!!.calendarHref && !it.isDisabled }
            }
        AlertDialog(
            onDismissRequest = { taskToMove = null },
            title = { Text("Move task") },
            text = {
                Column {
                    Text("Select destination:", fontSize = 14.sp, modifier = Modifier.padding(bottom = 8.dp))
                    LazyColumn {
                        items(targetCals) { cal ->
                            ListItem(
                                headlineContent = { Text(cal.name) },
                                leadingContent = { NfIcon(NfIcons.CALENDAR, 16.sp) },
                                modifier =
                                    Modifier.clickable {
                                        scope.launch {
                                            try {
                                                api.moveTask(taskToMove!!.uid, cal.href)
                                                taskToMove = null
                                                updateTaskList()
                                                onDataChanged()
                                            } catch (e: Exception) {
                                                Toast.makeText(context, "Move failed: ${e.message}", Toast.LENGTH_SHORT)
                                                    .show()
                                            }
                                        }
                                    },
                            )
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { taskToMove = null }) { Text("Cancel") } },
        )
    }

    if (showExportDialog) {
        AlertDialog(
            onDismissRequest = { showExportDialog = false },
            title = { Text("Export local tasks") },
            text = {
                Column {
                    Text("Select a destination calendar:", fontSize = 14.sp, modifier = Modifier.padding(bottom = 8.dp))
                    LazyColumn {
                        items(remoteCals) { cal ->
                            ListItem(
                                headlineContent = { Text(cal.name) },
                                leadingContent = { NfIcon(NfIcons.CALENDAR, 16.sp) },
                                modifier =
                                    Modifier.clickable {
                                        scope.launch {
                                            try {
                                                val msg = api.migrateLocalTo(cal.href)
                                                Toast.makeText(context, msg, Toast.LENGTH_SHORT).show()
                                                showExportDialog = false
                                                onGlobalRefresh()
                                            } catch (e: Exception) {
                                                Toast.makeText(
                                                    context,
                                                    "Export failed: ${e.message}",
                                                    Toast.LENGTH_SHORT
                                                ).show()
                                            }
                                        }
                                    },
                            )
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { showExportDialog = false }) { Text("Cancel") } },
        )
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet {
                Column(modifier = Modifier.fillMaxHeight().width(300.dp)) {
                    PrimaryTabRow(selectedTabIndex = sidebarTab) {
                        Tab(
                            selected = sidebarTab == 0,
                            onClick = { sidebarTab = 0 },
                            icon = { NfIcon(NfIcons.CALENDARS_VIEW) },
                        )
                        Tab(
                            selected = sidebarTab == 1,
                            onClick = { sidebarTab = 1 },
                            icon = { NfIcon(NfIcons.TAGS_VIEW) },
                        )
                        Tab(
                            selected = sidebarTab == 2,
                            onClick = { sidebarTab = 2 },
                            icon = { NfIcon(locationTabIcon) }, // <--- UPDATED HERE
                        )
                    }
                    LazyColumn(modifier = Modifier.weight(1f), contentPadding = PaddingValues(bottom = 24.dp)) {
                        if (sidebarTab == 0) {
                            item {
                                TextButton(
                                    onClick = {
                                        calendars.forEach { api.setCalendarVisibility(it.href, true) }
                                        onDataChanged()
                                        updateTaskList()
                                    },
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                ) { Text("Show all calendars") }
                                HorizontalDivider()
                            }
                            items(calendars.filter { !it.isDisabled }) { cal ->
                                val calColor = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                val isDefault = cal.href == defaultCalHref
                                val iconChar =
                                    if (isDefault) {
                                        NfIcons.WRITE_TARGET
                                    } else if (cal.isVisible) {
                                        NfIcons.VISIBLE
                                    } else {
                                        NfIcons.HIDDEN
                                    }
                                val iconColor = if (isDefault || cal.isVisible) calColor else Color.Gray
                                Row(
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    IconButton(onClick = {
                                        api.setCalendarVisibility(cal.href, !cal.isVisible)
                                        onDataChanged()
                                        updateTaskList()
                                    }) { NfIcon(iconChar, color = iconColor) }
                                    TextButton(
                                        onClick = {
                                            api.setDefaultCalendar(cal.href)
                                            onDataChanged()
                                        },
                                        modifier = Modifier.weight(1f),
                                        colors =
                                            ButtonDefaults.textButtonColors(
                                                contentColor = if (isDefault) calColor else MaterialTheme.colorScheme.onSurface,
                                            ),
                                    ) {
                                        Text(
                                            cal.name,
                                            fontWeight = if (isDefault) FontWeight.Bold else FontWeight.Normal,
                                            modifier = Modifier.fillMaxWidth(),
                                            textAlign = TextAlign.Start,
                                        )
                                    }
                                    IconButton(onClick = {
                                        scope.launch {
                                            api.isolateCalendar(cal.href)
                                            onDataChanged()
                                            drawerState.close()
                                        }
                                    }) { NfIcon(NfIcons.ARROW_RIGHT, size = 18.sp) }
                                }
                            }
                        } else if (sidebarTab == 1) {
                            item {
                                CompactTagRow(
                                    name = "All Tasks",
                                    count = null,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    isSelected = filterTag == null,
                                    onClick = {
                                        filterTag = null
                                        scope.launch { drawerState.close() }
                                    },
                                )
                            }
                            items(tags) { tag ->
                                val isUncat = tag.isUncategorized
                                val displayName = if (isUncat) "Uncategorized" else "#${tag.name}"
                                val isSel = if (isUncat) filterTag == ":::uncategorized:::" else filterTag == tag.name
                                val color = if (isUncat) Color.Gray else getTagColor(tag.name)
                                CompactTagRow(
                                    name = displayName,
                                    count = tag.count.toInt(),
                                    color = color,
                                    isSelected = isSel,
                                    onClick = {
                                        filterTag = if (isUncat) ":::uncategorized:::" else tag.name
                                        scope.launch { drawerState.close() }
                                    })
                            }
                        } else {
                            item {
                                CompactTagRow(
                                    name = "All Locations",
                                    count = null,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    isSelected = filterLocation == null,
                                    onClick = {
                                        filterLocation = null
                                        scope.launch { drawerState.close() }
                                    },
                                    icon = NfIcons.MAP,
                                )
                            }
                            items(locations) { loc ->
                                CompactTagRow(
                                    name = loc.name,
                                    count = loc.count.toInt(),
                                    color = Color(0xFFFFB300),
                                    isSelected = filterLocation == loc.name,
                                    onClick = {
                                        filterLocation = loc.name
                                        scope.launch { drawerState.close() }
                                    },
                                    icon = NfIcons.MAP_PIN,
                                )
                            }
                        }
                        item {
                            Box(
                                modifier = Modifier.fillMaxWidth().heightIn(min = 150.dp).padding(vertical = 32.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Image(
                                    painter = painterResource(id = R.drawable.ic_launcher_foreground),
                                    contentDescription = "Cfait Logo",
                                    modifier = Modifier.size(120.dp),
                                    contentScale = ContentScale.Fit,
                                )
                            }
                        }
                    }
                }
            }
        },
    ) {
        Scaffold(
            topBar = {
                if (isSearchActive) {
                    LaunchedEffect(Unit) {
                        searchFocusRequester.requestFocus()
                    }
                    TopAppBar(
                        title = {
                            TextField(
                                value = searchQuery,
                                onValueChange = { searchQuery = it },
                                placeholder = { Text("Search...") },
                                singleLine = true,
                                colors =
                                    TextFieldDefaults.colors(
                                        focusedContainerColor = Color.Transparent,
                                        unfocusedContainerColor = Color.Transparent,
                                        focusedIndicatorColor = Color.Transparent,
                                        unfocusedIndicatorColor = Color.Transparent,
                                    ),
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .focusRequester(searchFocusRequester),
                            )
                        },
                        navigationIcon = {
                            IconButton(onClick = {
                                isSearchActive = false
                                searchQuery = ""
                            }) { NfIcon(NfIcons.BACK, 20.sp) }
                        },
                    )
                } else {
                    val headerTitle: @Composable () -> Unit = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Image(
                                painter = painterResource(id = R.drawable.ic_launcher_foreground),
                                contentDescription = null,
                                modifier = Modifier.size(28.dp),
                            )
                            Spacer(Modifier.width(8.dp))
                            val activeCalName = calendars.find { it.href == defaultCalHref }?.name ?: "Local"
                            Text(
                                text = activeCalName,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f, fill = false),
                            )

                            // Count only active tasks
                            val activeCount = tasks.count { !it.isDone }

                            if (tasks.isNotEmpty()) {
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    text = "($activeCount)",
                                    fontSize = 13.sp,
                                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
                                )
                            }
                        }
                    }

                    TopAppBar(
                        title = headerTitle,
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                NfIcon(
                                    NfIcons.MENU,
                                    20.sp
                                )
                            }
                        },
                        actions = {
                            IconButton(onClick = { isSearchActive = true }) { NfIcon(NfIcons.SEARCH, 18.sp) }

                            if (isLoading || isManualSyncing || activeOpCount > 0 || isPullRefreshing) {
                                CircularProgressIndicator(modifier = Modifier.size(24.dp), strokeWidth = 2.dp)
                            } else {
                                val (icon, iconColor) =
                                    when {
                                        localHasUnsynced -> Pair(NfIcons.SYNC_ALERT, Color(0xFFEB0000))
                                        lastSyncFailed -> Pair(NfIcons.SYNC_OFF, Color(0xFFFFB300))
                                        else -> Pair(NfIcons.REFRESH, MaterialTheme.colorScheme.onSurface)
                                    }

                                IconButton(onClick = { handleRefresh() }) {
                                    NfIcon(icon, 18.sp, color = iconColor)
                                }
                            }
                            IconButton(onClick = onSettings) { NfIcon(NfIcons.SETTINGS, 20.sp) }
                        },
                    )
                }
            },
            bottomBar = {
                Column {
                    if (creatingChildTask != null) {
                        Surface(
                            color = MaterialTheme.colorScheme.tertiaryContainer,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Row(modifier = Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                NfIcon(NfIcons.CHILD, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer)
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    "New subtask for: ${creatingChildTask.summary}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onTertiaryContainer,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f),
                                )
                                IconButton(
                                    onClick = { creatingChildUid = null },
                                    modifier = Modifier.size(24.dp),
                                ) { NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer) }
                            }
                        }
                    } else if (yankedTask != null) {
                        Surface(
                            color = MaterialTheme.colorScheme.secondaryContainer,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Row(modifier = Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                NfIcon(NfIcons.LINK, 16.sp, MaterialTheme.colorScheme.onSecondaryContainer)
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    "Yanked: ${yankedTask.summary}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f),
                                )
                                IconButton(
                                    onClick = {
                                        yankedUid = null
                                    },
                                    modifier =
                                        Modifier.size(
                                            24.dp,
                                        ),
                                ) { NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.onSecondaryContainer) }
                            }
                        }
                    }
                    Surface(tonalElevation = 3.dp) {
                        Row(
                            Modifier.padding(16.dp).navigationBarsPadding(),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            OutlinedTextField(
                                value = newTaskText,
                                onValueChange = { newTaskText = it },
                                placeholder = { Text("!1 @tomorrow Buy cat food #groceries") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                visualTransformation = remember(isDark) { SmartSyntaxTransformation(api, isDark) },
                                keyboardOptions = KeyboardOptions.Default.copy(imeAction = ImeAction.Send),
                                keyboardActions = KeyboardActions(onSend = {
                                    if (newTaskText.isNotBlank()) addTask(
                                        newTaskText
                                    )
                                }),
                            )
                        }
                    }
                }
            },
        ) { padding ->
            Column(Modifier.padding(padding).fillMaxSize()) {
                val activeIsLocal = calendars.find { it.href == defaultCalHref }?.isLocal == true
                if (activeIsLocal && remoteCals.isNotEmpty()) {
                    FilledTonalButton(
                        onClick = { showExportDialog = true },
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                        colors =
                            ButtonDefaults.filledTonalButtonColors(
                                containerColor = MaterialTheme.colorScheme.tertiaryContainer,
                                contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
                            ),
                        contentPadding = PaddingValues(vertical = 8.dp),
                    ) {
                        NfIcon(NfIcons.EXPORT, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer)
                        Spacer(Modifier.width(8.dp))
                        Text("Export local tasks to server")
                    }
                }

                PullToRefreshBox(
                    isRefreshing = false,
                    onRefresh = { handlePullRefresh() },
                    modifier = Modifier.weight(1f),
                ) {
                    LazyColumn(
                        state = listState,
                        contentPadding = PaddingValues(bottom = 80.dp),
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        items(tasks, key = { it.uid }) { task ->
                            val calColor = calColorMap[task.calendarHref] ?: Color.Gray
                            TaskRow(
                                task = task,
                                calColor = calColor,
                                isDark = isDark,
                                onToggle = { toggleTask(task) },
                                onAction = { act -> onTaskAction(act, task) },
                                onClick = onTaskClick,
                                yankedUid = yankedUid,
                                enabledCalendarCount = enabledCalendarCount,
                            )
                        }
                    }
                }
            }
        }
    }
}
