// File: ./android/app/src/main/java/com/cfait/ui/HomeScreen.kt
package com.cfait.ui

import android.widget.Toast
import android.content.ClipData // FIXED: Correct Android Import
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
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.R
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileCalendar
import com.cfait.core.MobileTask
import com.cfait.core.MobileTag
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    api: CfaitMobile,
    calendars: List<MobileCalendar>,
    tags: List<MobileTag>,
    defaultCalHref: String?,
    isLoading: Boolean,
    hasUnsynced: Boolean,
    onGlobalRefresh: () -> Unit,
    onSettings: () -> Unit,
    // onHelp removed
    onTaskClick: (String) -> Unit,
    onDataChanged: () -> Unit
) {
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var sidebarTab by remember { mutableIntStateOf(0) }
    
    val listState = rememberLazyListState()
    
    var tasks by remember { mutableStateOf<List<MobileTask>>(emptyList()) }
    var searchQuery by remember { mutableStateOf("") }
    var filterTag by remember { mutableStateOf<String?>(null) }
    var isSearchActive by remember { mutableStateOf(false) }
    var newTaskText by remember { mutableStateOf("") }
    
    var showExportDialog by remember { mutableStateOf(false) }
    
    var yankedUid by remember { mutableStateOf<String?>(null) }
    val yankedTask = remember(tasks, yankedUid) { tasks.find { it.uid == yankedUid } }
    
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val isDark = isSystemInDarkTheme()

    val calColorMap = remember(calendars) { 
        calendars.associate { it.href to (it.color?.let { hex -> parseHexColor(hex) } ?: Color.Gray) }
    }

    BackHandler(enabled = drawerState.isOpen) {
        scope.launch { drawerState.close() }
    }

    suspend fun fetchTasks(): List<MobileTask> {
        return try { api.getViewTasks(filterTag, searchQuery) } catch (_: Exception) { emptyList() }
    }

    fun updateTaskList() {
        scope.launch { tasks = fetchTasks() }
    }

    LaunchedEffect(searchQuery, filterTag, isLoading, calendars, tags) { updateTaskList() }

    fun toggleTask(uid: String) = scope.launch { try { api.toggleTask(uid); updateTaskList(); onDataChanged() } catch (_: Exception){} }
    
    fun addTask(txt: String) {
        val text = txt.trim()
        if (text.startsWith("#") && !text.contains(" ")) {
            val tag = text.removePrefix("#")
            filterTag = tag
            sidebarTab = 1
            newTaskText = ""
            scope.launch { 
                tasks = fetchTasks() 
            }
        } else {
            scope.launch {
                try {
                    val newUid = api.addTaskSmart(text)
                    newTaskText = ""
                    onDataChanged() 
                    
                    val newTasks = fetchTasks()
                    tasks = newTasks
                    
                    val index = newTasks.indexOfFirst { it.uid == newUid }
                    if (index >= 0) {
                        listState.animateScrollToItem(index)
                    }
                } catch (_: Exception){}
            }
        }
    }
    
    fun onTaskAction(action: String, task: MobileTask) {
        scope.launch {
            try {
                when(action) {
                    "delete" -> api.deleteTask(task.uid)
                    "cancel" -> api.setStatusCancelled(task.uid)
                    "playpause" -> api.setStatusProcess(task.uid)
                    "prio_up" -> api.changePriority(task.uid, 1)
                    "prio_down" -> api.changePriority(task.uid, -1)
                    "yank" -> {
                        yankedUid = task.uid
                        // FIXED: Use Android ClipData
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
            } catch(_: Exception) {}
        }
    }

    val remoteCals = remember(calendars) { calendars.filter { !it.isLocal && !it.isDisabled } }
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
                                modifier = Modifier
                                    .clickable {
                                        scope.launch {
                                            try {
                                                val msg = api.migrateLocalTo(cal.href)
                                                Toast.makeText(context, msg, Toast.LENGTH_SHORT).show()
                                                showExportDialog = false
                                                onGlobalRefresh()
                                            } catch (e: Exception) {
                                                Toast.makeText(context, "Export failed: ${e.message}", Toast.LENGTH_SHORT).show()
                                            }
                                        }
                                    }
                            )
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { showExportDialog = false }) { Text("Cancel") } }
        )
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet {
                Column(modifier = Modifier.fillMaxHeight().width(300.dp)) {
                    PrimaryTabRow(selectedTabIndex = sidebarTab) {
                        Tab(selected = sidebarTab==0, onClick = { sidebarTab=0 }, text = { Text("Calendars") }, icon = { NfIcon(NfIcons.CALENDAR) })
                        Tab(selected = sidebarTab==1, onClick = { sidebarTab=1 }, text = { Text("Tags") }, icon = { NfIcon(NfIcons.TAG) })
                    }
                    LazyColumn(
                        modifier = Modifier.weight(1f),
                        contentPadding = PaddingValues(bottom = 24.dp)
                    ) {
                        if (sidebarTab == 0) {
                            item {
                                TextButton(
                                    onClick = { 
                                        calendars.forEach { api.setCalendarVisibility(it.href, true) }
                                        onDataChanged()
                                        updateTaskList()
                                    },
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp)
                                ) {
                                    Text("Show all calendars")
                                }
                                HorizontalDivider()
                            }

                            items(calendars.filter { !it.isDisabled }) { cal ->
                                val calColor = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                val isDefault = cal.href == defaultCalHref
                                val iconChar = if (isDefault) NfIcons.WRITE_TARGET else if (cal.isVisible) NfIcons.VISIBLE else NfIcons.HIDDEN
                                val iconColor = if (isDefault || cal.isVisible) calColor else Color.Gray

                                Row(
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    IconButton(onClick = { api.setCalendarVisibility(cal.href, !cal.isVisible); onDataChanged(); updateTaskList() }) {
                                        NfIcon(iconChar, color = iconColor)
                                    }
                                    TextButton(
                                        onClick = {
                                            api.setDefaultCalendar(cal.href)
                                            onDataChanged()
                                        },
                                        modifier = Modifier.weight(1f),
                                        colors = ButtonDefaults.textButtonColors(contentColor = if (isDefault) calColor else MaterialTheme.colorScheme.onSurface)
                                    ) {
                                        Text(
                                            cal.name,
                                            fontWeight = if (isDefault) FontWeight.Bold else FontWeight.Normal,
                                            modifier = Modifier.fillMaxWidth(),
                                            textAlign = TextAlign.Start
                                        )
                                    }
                                    IconButton(onClick = {
                                        scope.launch {
                                            api.isolateCalendar(cal.href)
                                            onDataChanged()
                                            drawerState.close()
                                        }
                                    }) {
                                        NfIcon(NfIcons.ARROW_RIGHT, size = 18.sp)
                                    }
                                }
                            }
                        } else {
                            item {
                                CompactTagRow(
                                    name = "All Tasks",
                                    count = null,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    isSelected = filterTag == null,
                                    onClick = { filterTag = null; scope.launch { drawerState.close() } }
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
                                    }
                                )
                            }
                        }

                        item {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .heightIn(min = 150.dp)
                                    .padding(vertical = 32.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Image(
                                    painter = painterResource(id = R.drawable.ic_launcher_foreground),
                                    contentDescription = "Cfait Logo",
                                    modifier = Modifier.size(120.dp),
                                    contentScale = ContentScale.Fit
                                )
                            }
                        }
                    }
                }
            }
        }
    ) {
        Scaffold(
            topBar = {
                if (isSearchActive) {
                    TopAppBar(
                        title = { TextField(value = searchQuery, onValueChange = { searchQuery = it }, placeholder = { Text("Search...") }, singleLine = true, colors = TextFieldDefaults.colors(focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent, focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent), modifier = Modifier.fillMaxWidth()) },
                        navigationIcon = { IconButton(onClick = { isSearchActive = false; searchQuery = "" }) { NfIcon(NfIcons.BACK, 20.sp) } }
                    )
                } else {
                    val headerTitle: @Composable () -> Unit = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Image(
                                painter = painterResource(id = R.drawable.ic_launcher_foreground),
                                contentDescription = null,
                                modifier = Modifier.size(28.dp)
                            )
                            Spacer(Modifier.width(8.dp))
                            
                            val activeCalName = calendars.find { it.href == defaultCalHref }?.name ?: "Local"
                            
                            Text(
                                text = activeCalName,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f, fill = false)
                            )
                            
                            if (tasks.isNotEmpty()) {
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    text = "(${tasks.size})",
                                    fontSize = 13.sp,
                                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f)
                                )
                            }
                        }
                    }

                    TopAppBar(
                        title = headerTitle,
                        navigationIcon = { IconButton(onClick = { scope.launch { drawerState.open() } }) { NfIcon(NfIcons.MENU, 20.sp) } },
                        actions = {
                            IconButton(onClick = { isSearchActive = true }) { NfIcon(NfIcons.SEARCH, 18.sp) }
                            if (isLoading) {
                                CircularProgressIndicator(modifier = Modifier.size(24.dp), strokeWidth = 2.dp)
                            } else {
                                if (hasUnsynced) {
                                    NfIcon(NfIcons.UNSYNCED, 18.sp, color = MaterialTheme.colorScheme.primary)
                                }
                                IconButton(onClick = onGlobalRefresh) { NfIcon(NfIcons.REFRESH, 18.sp) }
                            }
                            IconButton(onClick = onSettings) { NfIcon(NfIcons.SETTINGS, 20.sp) }
                        }
                    )
                }
            },
            bottomBar = {
                Column {
                    if (yankedTask != null) {
                        Surface(
                            color = MaterialTheme.colorScheme.secondaryContainer,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Row(
                                modifier = Modifier.padding(8.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                NfIcon(NfIcons.LINK, 16.sp, MaterialTheme.colorScheme.onSecondaryContainer)
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    "Yanked: ${yankedTask.summary}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f)
                                )
                                IconButton(onClick = { yankedUid = null }, modifier = Modifier.size(24.dp)) {
                                    NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.onSecondaryContainer)
                                }
                            }
                        }
                    }
                    Surface(tonalElevation = 3.dp) {
                        Row(Modifier.padding(16.dp).navigationBarsPadding(), verticalAlignment = Alignment.CenterVertically) {
                            OutlinedTextField(
                                value = newTaskText,
                                onValueChange = { newTaskText = it },
                                placeholder = { Text("!1 @tomorrow Buy cat food #groceries") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                keyboardOptions = KeyboardOptions.Default.copy(imeAction = ImeAction.Send),
                                keyboardActions = KeyboardActions(onSend = {
                                    if (newTaskText.isNotBlank()) {
                                        addTask(newTaskText)
                                    }
                                })
                            )
                        }
                    }
                }
            }
        ) { padding ->
            Column(Modifier.padding(padding).fillMaxSize()) {
                
                val activeIsLocal = calendars.find { it.href == defaultCalHref }?.isLocal == true
                if (activeIsLocal && remoteCals.isNotEmpty()) {
                    FilledTonalButton(
                        onClick = { showExportDialog = true },
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                        colors = ButtonDefaults.filledTonalButtonColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer, contentColor = MaterialTheme.colorScheme.onTertiaryContainer),
                        contentPadding = PaddingValues(vertical = 8.dp)
                    ) {
                        NfIcon(NfIcons.EXPORT, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer)
                        Spacer(Modifier.width(8.dp))
                        Text("Export local tasks to server")
                    }
                }

                LazyColumn(
                    modifier = Modifier.weight(1f),
                    state = listState,
                    contentPadding = PaddingValues(bottom = 80.dp)
                ) {
                    items(tasks, key = { it.uid }) { task ->
                        val calColor = calColorMap[task.calendarHref] ?: Color.Gray
                        TaskRow(
                            task = task, 
                            calColor = calColor, 
                            isDark = isDark, 
                            onToggle = { toggleTask(task.uid) }, 
                            onAction = { act -> onTaskAction(act, task) }, 
                            onClick = onTaskClick,
                            yankedUid = yankedUid
                        )
                    }
                }
            }
        }
    }
}