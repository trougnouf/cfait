// File: android/app/src/main/java/com/cfait/ui/HomeScreen.kt
package com.cfait.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboardManager
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
    hasUnsynced: Boolean, // <-- ADDED THIS PARAMETER
    onGlobalRefresh: () -> Unit,
    onSettings: () -> Unit,
    onTaskClick: (String) -> Unit,
    onDataChanged: () -> Unit
) {
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    var sidebarTab by remember { mutableIntStateOf(0) }
    
    var tasks by remember { mutableStateOf<List<MobileTask>>(emptyList()) }
    var searchQuery by remember { mutableStateOf("") }
    var filterTag by remember { mutableStateOf<String?>(null) }
    var isSearchActive by remember { mutableStateOf(false) }
    var newTaskText by remember { mutableStateOf("") }
    
    // Yank State
    var yankedUid by remember { mutableStateOf<String?>(null) }
    val yankedTask = remember(tasks, yankedUid) { tasks.find { it.uid == yankedUid } }
    
    val clipboardManager = LocalClipboardManager.current
    val isDark = isSystemInDarkTheme()

    val calColorMap = remember(calendars) { 
        calendars.associate { it.href to (it.color?.let { hex -> parseHexColor(hex) } ?: Color.Gray) }
    }

    BackHandler(enabled = drawerState.isOpen) {
        scope.launch { drawerState.close() }
    }

    fun updateTaskList() {
        scope.launch { try { tasks = api.getViewTasks(filterTag, searchQuery) } catch (_: Exception) { } }
    }

    LaunchedEffect(searchQuery, filterTag, isLoading, calendars, tags) { updateTaskList() }

    fun toggleTask(uid: String) = scope.launch { try { api.toggleTask(uid); updateTaskList(); onDataChanged() } catch (_: Exception){} }
    fun addTask(txt: String) = scope.launch { try { api.addTaskSmart(txt); updateTaskList(); onDataChanged() } catch (_: Exception){} }
    
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
                        clipboardManager.setText(AnnotatedString(task.uid))
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
                            items(calendars.filter { !it.isDisabled }) { cal ->
                                val calColor = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                val isDefault = cal.href == defaultCalHref
                                val iconChar = if (isDefault) NfIcons.WRITE_TARGET else if (cal.isVisible) NfIcons.VISIBLE else NfIcons.HIDDEN
                                val iconColor = if (isDefault) MaterialTheme.colorScheme.primary else if (cal.isVisible) calColor else Color.Gray

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
                                        colors = ButtonDefaults.textButtonColors(contentColor = if (isDefault) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
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
                                placeholder = { Text("!1 @tomorrow Buy milk") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                keyboardOptions = KeyboardOptions.Default.copy(imeAction = ImeAction.Send),
                                keyboardActions = KeyboardActions(onSend = {
                                    if (newTaskText.isNotBlank()) {
                                        addTask(newTaskText)
                                        newTaskText = ""
                                    }
                                })
                            )
                        }
                    }
                }
            }
        ) { padding ->
            LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 80.dp)) {
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