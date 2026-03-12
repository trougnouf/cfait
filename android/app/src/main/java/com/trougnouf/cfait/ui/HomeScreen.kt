package com.trougnouf.cfait.ui

import android.content.ClipData
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.material3.TabRowDefaults.tabIndicatorOffset
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.*
import kotlinx.coroutines.launch

data class TabInfo(
    val id: String,
    val name: String,
    val hrefs: Set<String>,
    val color: Color?,
    val isWriteTarget: String?
)

@Composable
fun ColoredOverflowDots() {
    Row(verticalAlignment = Alignment.Bottom) {
        Text(".", color = Color(0xFFFF4444), fontSize = 13.sp)
        Text(".", color = Color(0xFF66BB6A), fontSize = 13.sp)
        Text(".", color = Color(0xFF42A5F5), fontSize = 13.sp)
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun HomeScreen(
    api: CfaitMobile,
    calendars: List<MobileCalendar>,
    defaultCalHref: String?,
    defaultPriority: Int,
    isLoading: Boolean,
    hasUnsynced: Boolean,
    autoScrollUid: String? = null,
    refreshTick: Long,
    tabPosition: String,
    tabAutoHide: Boolean = true,
    onGlobalRefresh: () -> Unit,
    onSettings: () -> Unit,
    onTaskClick: (String) -> Unit,
    onDataChanged: () -> Unit,
    onMigrateLocal: (String, String) -> Unit,
    onAutoScrollComplete: () -> Unit = {},
) {
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboard = LocalClipboard.current
    val keyboardController = LocalSoftwareKeyboardController.current
    val isDark = isSystemInDarkTheme()
    val hapticFeedback = LocalHapticFeedback.current

    // --- State Declarations ---
    var sidebarTab by rememberSaveable { mutableIntStateOf(0) }
    var isManualSyncing by remember { mutableStateOf(false) }
    var activeOpCount by remember { mutableIntStateOf(0) }
    var lastSyncFailed by remember { mutableStateOf(false) }
    var localHasUnsynced by remember { mutableStateOf(hasUnsynced) }
    var isPullRefreshing by remember { mutableStateOf(false) }

    val enabledCals = remember(calendars) { calendars.filter { !it.isDisabled && it.href != "local://trash" } }
    val allHrefs = remember(enabledCals) { enabledCals.map { it.href }.toSet() }
    val backendVisibleHrefs = remember(enabledCals) { enabledCals.filter { it.isVisible }.map { it.href }.toSet() }

    var customHrefs by rememberSaveable { mutableStateOf<Set<String>>(emptySet()) }
    var customWriteTarget by rememberSaveable { mutableStateOf<String?>(null) }
    var localDefaultCalHref by remember(defaultCalHref) { mutableStateOf(defaultCalHref) }

    var hasInitializedCustom by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(backendVisibleHrefs) {
        if (!hasInitializedCustom && backendVisibleHrefs.size > 1 && backendVisibleHrefs.size < allHrefs.size) {
            customHrefs = backendVisibleHrefs
            hasInitializedCustom = true
        }
    }

    // Stable tabs list: All -> Custom -> Rest
    val tabs = remember(enabledCals.map { it.href }, customHrefs, customWriteTarget, allHrefs) {
        val list = mutableListOf<TabInfo>()
        list.add(TabInfo("ALL", "All", allHrefs, null, null))
        if (customHrefs.isNotEmpty() && customHrefs.size < allHrefs.size) {
            list.add(TabInfo("CUSTOM", "Custom", customHrefs, null, customWriteTarget))
        }
        enabledCals.forEach { cal ->
            list.add(TabInfo(cal.href, cal.name, setOf(cal.href), cal.color?.let { parseHexColor(it) }, cal.href))
        }
        list
    }

    val initialPage = remember(tabs, localDefaultCalHref) {
        val idx = tabs.indexOfFirst { it.id == localDefaultCalHref }
        if (idx >= 0) idx else if (customHrefs.isNotEmpty()) tabs.indexOfFirst { it.id == "CUSTOM" } else 0
    }

    val pagerState = rememberPagerState(initialPage = initialPage, pageCount = { tabs.size })
    var pendingTabId by remember { mutableStateOf<String?>(null) }

    var isTabsTemporarilyVisible by remember { mutableStateOf(false) }
    LaunchedEffect(pagerState.isScrollInProgress) {
        if (pagerState.isScrollInProgress) {
            isTabsTemporarilyVisible = true
        } else {
            kotlinx.coroutines.delay(2500)
            isTabsTemporarilyVisible = false
        }
    }

    val showTabs = enabledCals.size > 1 && (!tabAutoHide || isTabsTemporarilyVisible)

    var tasks by remember { mutableStateOf<List<MobileTask>>(emptyList()) }
    var tags by remember { mutableStateOf<List<MobileTag>>(emptyList()) }
    var locations by remember { mutableStateOf<List<MobileLocation>>(emptyList()) }

    // Local cache for instant swiping: Accumulate over time
    var taskCache by remember { mutableStateOf<Map<String, List<MobileTask>>>(emptyMap()) }
    LaunchedEffect(tasks) {
        val newCache = taskCache.toMutableMap()
        val groupedTasks = tasks.groupBy { it.calendarHref }

        // Update populated calendars
        groupedTasks.forEach { (href, hrefTasks) ->
            newCache[href] = hrefTasks
        }

        // Find calendars that were active in the current tab but have 0 tasks now
        val currentTab = tabs.getOrNull(pagerState.currentPage)
        currentTab?.hrefs?.forEach { href ->
            if (!groupedTasks.containsKey(href)) {
                newCache[href] = emptyList() // Clear ghosts
            }
        }
        taskCache = newCache
    }

    var filterTags by rememberSaveable { mutableStateOf<Set<String>>(emptySet()) }
    var filterLocations by rememberSaveable { mutableStateOf<Set<String>>(emptySet()) }
    var matchAllCategories by rememberSaveable { mutableStateOf(true) }
    var expandedGroups by rememberSaveable { mutableStateOf<Set<String>>(emptySet()) }

    var searchQuery by rememberSaveable { mutableStateOf("") }
    var isSearchActive by rememberSaveable { mutableStateOf(false) }
    val searchFocusRequester = remember { FocusRequester() }
    var hasRequestedSearchFocus by rememberSaveable { mutableStateOf(false) }

    var taskToMove by remember { mutableStateOf<MobileTask?>(null) }
    var aliases by remember { mutableStateOf<Map<String, List<String>>>(emptyMap()) }
    var childLockActive by rememberSaveable { mutableStateOf(false) }
    var yankLockActive by rememberSaveable { mutableStateOf(false) }

    var highlightedUid by remember { mutableStateOf(autoScrollUid) }
    var scrollTrigger by remember { mutableLongStateOf(0L) }

    var newTaskText by remember { mutableStateOf("") }
    var showExportSourceDialog by remember { mutableStateOf(false) }
    var showExportDestDialog by remember { mutableStateOf(false) }
    var exportSourceHref by remember { mutableStateOf<String?>(null) }

    var yankedUid by remember { mutableStateOf<String?>(null) }
    val yankedTask = remember(tasks, yankedUid) { tasks.find { it.uid == yankedUid } }
    var creatingChildUid by remember { mutableStateOf<String?>(null) }
    val creatingChildTask = remember(tasks, creatingChildUid) { tasks.find { it.uid == creatingChildUid } }

    val listStates = remember { mutableStateMapOf<String, LazyListState>() }
    val activeListState = remember(pagerState.currentPage, tabs) {
        val key = tabs.getOrNull(pagerState.currentPage)?.id ?: "ALL_TASKS"
        listStates.getOrPut(key) { LazyListState() }
    }

    var showScrollToTop by remember { mutableStateOf(false) }
    var lastScrollPosition by remember { mutableIntStateOf(0) }
    var isProgrammaticScroll by remember { mutableStateOf(false) }
    val scrollToTopIcon = remember { getRandomScrollToTopIcon() }

    val randomIcons = remember {
        listOf(
            NfIcons.DICE_D20, NfIcons.DICE_D20_DUP, NfIcons.DICE_D6, NfIcons.DICE_MULTIPLE,
            NfIcons.AUTO_FIX, NfIcons.CRYSTAL_BALL, NfIcons.ATOM, NfIcons.CAT,
            NfIcons.CAT_MD, NfIcons.UNICORN, NfIcons.UNICORN_VARIANT, NfIcons.RAINBOW,
            NfIcons.FRUIT_CHERRIES, NfIcons.FRUIT_PINEAPPLE, NfIcons.FRUIT_PEAR, NfIcons.DOG,
            NfIcons.PHOENIX, NfIcons.LINUX, NfIcons.TORTOISE, NfIcons.FACE_SMILE_WINK,
            NfIcons.ROBOT_LOVE_OUTLINE, NfIcons.BOW_ARROW, NfIcons.BULLSEYE_ARROW, NfIcons.COINS,
            NfIcons.COW, NfIcons.DOLPHIN, NfIcons.KIWI_BIRD, NfIcons.DUCK,
            NfIcons.FAE_TREE, NfIcons.FA_TREE, NfIcons.MD_TREE, NfIcons.PLANT,
            NfIcons.WIZARD_HAT, NfIcons.STAR_SHOOTING_OUTLINE, NfIcons.WEATHER_STARS, NfIcons.KOALA,
            NfIcons.SPIDER_THREAD, NfIcons.SQUIRREL, NfIcons.MUSHROOM_OUTLINE, NfIcons.FLOWER,
            NfIcons.BEE_FLOWER, NfIcons.LINUX_FREEBSD, NfIcons.BUG, NfIcons.WEATHER_SUNNY,
            NfIcons.FROG, NfIcons.BINOCULARS, NfIcons.ORANGE, NfIcons.SNOWMAN,
            NfIcons.GNU, NfIcons.RUST, NfIcons.R_BOX, NfIcons.PEPPER_HOT, NfIcons.SIGN_POST
        )
    }
    var currentRandomIcon by remember { mutableStateOf(randomIcons.random()) }

    val locationTabIcon = rememberSaveable {
        val icons = listOf(
            NfIcons.LOCATION, NfIcons.EARTH_ASIA, NfIcons.EARTH_AMERICAS,
            NfIcons.EARTH_AFRICA, NfIcons.EARTH_GENERIC, NfIcons.PLANET,
            NfIcons.GALAXY, NfIcons.ISLAND, NfIcons.COMPASS,
            NfIcons.MOUNTAINS, NfIcons.GLOBE, NfIcons.GLOBEMODEL,
        )
        icons.random()
    }

    val enabledCalendarCount = remember(calendars) { calendars.count { !it.isDisabled } }
    val calColorMap = remember(calendars) {
        calendars.associate { it.href to (it.color?.let { hex -> parseHexColor(hex) } ?: Color.Gray) }
    }
    val taskMap = remember(tasks) { tasks.associateBy { it.uid } }
    val incomingRelationsMap = remember(tasks) {
        val map = mutableMapOf<String, MutableList<String>>()
        tasks.forEach { task ->
            task.relatedToUids.forEach { relatedUid ->
                map.getOrPut(relatedUid) { mutableListOf() }.add(task.uid)
            }
        }
        map
    }

    var hasSetDefaultTab by rememberSaveable { mutableStateOf(false) }

    val onSurfaceColor = MaterialTheme.colorScheme.onSurface
    val activeColor by derivedStateOf {
        val pageOffset = pagerState.currentPageOffsetFraction
        val currentIndex = pagerState.currentPage
        val targetIndex = if (pageOffset < 0) currentIndex - 1 else currentIndex + 1
        val safeTarget = targetIndex.coerceIn(0, tabs.lastIndex)

        val c1 = tabs.getOrNull(currentIndex)?.color ?: onSurfaceColor
        val c2 = tabs.getOrNull(safeTarget)?.color ?: onSurfaceColor
        lerp(c1, c2, kotlin.math.abs(pageOffset))
    }

    // --- Functions ---
    fun updateTaskList() {
        scope.launch {
            try {
                val viewData = api.getViewTasks(
                    filterTags.toList(), filterLocations.toList(), searchQuery,
                    expandedGroups.toList(), matchAllCategories
                )
                tasks = viewData.tasks
                tags = viewData.tags
                locations = viewData.locations
                aliases = api.getConfig().tagAliases
            } catch (_: Exception) {
            }
        }
    }

    fun checkSyncStatus() {
        scope.launch {
            try {
                localHasUnsynced = api.hasUnsyncedChanges()
            } catch (_: Exception) {
            }
        }
    }

    fun jumpToRandomTask() {
        if (tasks.isEmpty()) return
        currentRandomIcon = randomIcons.random()
        scope.launch {
            val currentTab = tabs.getOrNull(pagerState.currentPage) ?: return@launch
            val validTasks = tasks.filter {
                it.calendarHref in currentTab.hrefs && !it.isDone && !it.isBlocked && !it.isFutureStart
            }
            if (validTasks.isEmpty()) return@launch

            val totalWeight = validTasks.sumOf {
                val p = if (it.priority.toInt() == 0) defaultPriority else it.priority.toInt()
                (10 - p).coerceIn(1, 9)
            }
            var rnd = (0 until totalWeight).random()
            var targetTask = validTasks.last()
            for (t in validTasks) {
                val p = if (t.priority.toInt() == 0) defaultPriority else t.priority.toInt()
                val w = (10 - p).coerceIn(1, 9)
                if (rnd < w) {
                    targetTask = t
                    break
                }
                rnd -= w
            }
            highlightedUid = targetTask.uid
            scrollTrigger++
        }
    }

    fun toggleTask(task: MobileTask) {
        val newIsDone = !task.isDone
        val newStatus = if (newIsDone) "Completed" else "NeedsAction"
        tasks = tasks.map {
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
            filterTags = setOf(tag)
            sidebarTab = 1
            newTaskText = ""
            updateTaskList()
        } else if ((text.startsWith("@@") || text.startsWith("loc:")) && !text.contains(" ") && !isAliasDef) {
            val loc = if (text.startsWith("@@")) text.removePrefix("@@") else text.removePrefix("loc:")
            filterLocations = setOf(loc.replace("\"", ""))
            sidebarTab = 2
            newTaskText = ""
            updateTaskList()
        } else {
            newTaskText = ""
            scope.launch {
                activeOpCount++
                try {
                    // FIX: Ensure the backend knows the correct target immediately,
                    // even if the user typed this while the page was still settling.
                    val activeTab = tabs.getOrNull(pagerState.currentPage)
                    if (activeTab?.isWriteTarget != null && activeTab.isWriteTarget != defaultCalHref) {
                        api.setDefaultCalendar(activeTab.isWriteTarget)
                    }

                    val newUid = api.addTaskSmart(text)
                    if (creatingChildUid != null) {
                        api.setParent(newUid, creatingChildUid!!)
                        if (!childLockActive) creatingChildUid = null
                    }
                    highlightedUid = newUid
                    onDataChanged()
                    lastSyncFailed = false
                    updateTaskList()
                    scrollTrigger++
                } catch (_: Exception) {
                    lastSyncFailed = true
                } finally {
                    checkSyncStatus()
                    activeOpCount--
                }
            }
        }
    }

    fun onTaskAction(action: String, task: MobileTask) {
        if (action == "move") {
            taskToMove = task; return
        }
        if (action == "create_child") {
            creatingChildUid = task.uid
            yankedUid = null
            val sb = StringBuilder()
            fun quote(s: String): String =
                if (s.contains(" ") || s.contains("\"")) "\"${s.replace("\"", "\\\"")}\"" else s
            task.categories.forEach { cat -> sb.append("#${quote(cat)} ") }
            task.location?.let { loc -> sb.append("@@${quote(loc)} ") }
            newTaskText = sb.toString()
            return
        }

        val updatedList = tasks.map { t ->
            if (t.uid == task.uid) {
                when (action) {
                    "delete" -> null
                    "cancel" -> t.copy(statusString = "Cancelled", isDone = true)
                    "playpause" -> if (t.statusString == "InProcess") t.copy(
                        statusString = "NeedsAction",
                        isPaused = true
                    )
                    else t.copy(statusString = "InProcess", isPaused = false)

                    "stop" -> t.copy(statusString = "NeedsAction", isPaused = false)
                    "prio_up" -> {
                        var p = t.priority.toInt()
                        if (p == 0) p = (defaultPriority - 1).coerceAtLeast(1) else if (p > 1) p -= 1
                        t.copy(priority = p.toUByte())
                    }

                    "prio_down" -> {
                        var p = t.priority.toInt()
                        if (p == 0) p = (defaultPriority + 1).coerceAtMost(9) else if (p < 9) p += 1
                        t.copy(priority = p.toUByte())
                    }

                    else -> t
                }
            } else t
        }.filterNotNull()
        tasks = updatedList

        scope.launch {
            activeOpCount++
            try {
                when (action) {
                    "delete" -> api.deleteTask(task.uid)
                    "cancel" -> api.setStatusCancelled(task.uid)
                    "playpause" -> if (task.statusString == "InProcess") api.pauseTask(task.uid) else api.startTask(task.uid)
                    "stop" -> api.stopTask(task.uid)
                    "prio_up" -> api.changePriority(task.uid, 1)
                    "prio_down" -> api.changePriority(task.uid, -1)
                    "yank" -> {
                        yankedUid = task.uid
                        val textToCopy =
                            if (task.description.isEmpty()) task.smartString else "${task.smartString}\n\n${task.description}"
                        clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("task_details", textToCopy)))
                    }

                    "block" -> if (yankedUid != null) {
                        api.addDependency(task.uid, yankedUid!!); if (!yankLockActive) yankedUid = null
                    }

                    "child" -> if (yankedUid != null) {
                        api.setParent(task.uid, yankedUid!!); if (!yankLockActive) yankedUid = null
                    }

                    "related" -> if (yankedUid != null) {
                        api.addRelatedTo(task.uid, yankedUid!!); if (!yankLockActive) yankedUid = null
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
                Toast.makeText(context, context.getString(R.string.sync_error, e.message ?: ""), Toast.LENGTH_SHORT)
                    .show()
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
                Toast.makeText(context, context.getString(R.string.sync_error, e.message ?: ""), Toast.LENGTH_SHORT)
                    .show()
                api.loadFromCache()
                updateTaskList()
            } finally {
                checkSyncStatus()
                isPullRefreshing = false
            }
        }
    }

    // --- Effects ---

    LaunchedEffect(autoScrollUid) {
        if (autoScrollUid != null) highlightedUid = autoScrollUid
    }

    LaunchedEffect(hasUnsynced) { localHasUnsynced = hasUnsynced }

    LaunchedEffect(pendingTabId, tabs) {
        if (pendingTabId != null) {
            val idx = tabs.indexOfFirst { it.id == pendingTabId }
            if (idx >= 0 && pagerState.currentPage != idx) {
                pagerState.scrollToPage(idx)
            }
            pendingTabId = null
        }
    }

    // Eagerly update backend settings ONLY when the swipe settles.
    LaunchedEffect(pagerState.settledPage) {
        if (tabs.isEmpty() || pagerState.isScrollInProgress) return@LaunchedEffect
        val settledTab = tabs.getOrNull(pagerState.settledPage) ?: return@LaunchedEffect

        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            // FIX: Wait a fraction of a second so the Compose gesture subsystem
            // fully releases the horizontal lock. This completely eliminates
            // the delay preventing immediate vertical scrolling.
            kotlinx.coroutines.delay(250)

            var needsRefresh = false
            enabledCals.forEach { cal ->
                val shouldBeVisible = settledTab.hrefs.contains(cal.href)
                if (cal.isVisible != shouldBeVisible) {
                    api.setCalendarVisibility(cal.href, shouldBeVisible)
                    needsRefresh = true
                }
            }

            if (settledTab.isWriteTarget != null && settledTab.isWriteTarget != defaultCalHref) {
                api.setDefaultCalendar(settledTab.isWriteTarget)
                needsRefresh = true
            }

            if (needsRefresh) {
                updateTaskList()
                onDataChanged() // Let parent know config changed
            }
        }
    }

    LaunchedEffect(pagerState.currentPage) {
        if (pagerState.currentPage != pagerState.targetPage) {
            hapticFeedback.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        }
    }

    LaunchedEffect(calendars) {
        if (!hasSetDefaultTab && calendars.isNotEmpty()) {
            val hasRemote = calendars.any { !it.isLocal }
            if (!hasRemote) sidebarTab = 1
            hasSetDefaultTab = true
        }
    }

    LaunchedEffect(
        searchQuery, filterTags, filterLocations, isLoading,
        calendars, refreshTick, expandedGroups, matchAllCategories
    ) {
        updateTaskList()
    }

    LaunchedEffect(scrollTrigger, autoScrollUid, tasks) {
        if (highlightedUid == null || tasks.isEmpty()) return@LaunchedEffect
        // Only run if an explicit jump or intent was just requested
        if (scrollTrigger == 0L && autoScrollUid == null) return@LaunchedEffect

        try {
            val targetTask = tasks.find { it.uid == highlightedUid } ?: return@LaunchedEffect

            val currentTab = tabs.getOrNull(pagerState.currentPage)
            val needsTabSwitch = currentTab != null && !currentTab.hrefs.contains(targetTask.calendarHref)

            // 1. Jump Tab if needed
            if (needsTabSwitch) {
                val tabIndex = tabs.indexOfFirst { it.isWriteTarget == targetTask.calendarHref }
                if (tabIndex >= 0) {
                    // This naturally suspends the coroutine until the animation completes
                    pagerState.animateScrollToPage(tabIndex)
                } else {
                    pagerState.animateScrollToPage(0)
                }
            }

            // 2. Find the index in the new tab's list
            val activeTab = tabs.getOrNull(pagerState.currentPage)
            val currentList = if (activeTab != null) {
                tasks.filter { it.calendarHref in activeTab.hrefs }
            } else tasks

            val index = currentList.indexOfFirst { it.uid == highlightedUid }

            // 3. Scroll the list
            if (index >= 0) {
                val key = tabs.getOrNull(pagerState.currentPage)?.id ?: "ALL_TASKS"
                // Ensure the list state exists so we don't miss the scroll on unvisited tabs
                val listState = listStates.getOrPut(key) { LazyListState() }

                if (scrollTrigger > 0) {
                    listState.scrollToItem(index)
                } else if (autoScrollUid != null) {
                    listState.animateScrollToItem(index)
                    kotlinx.coroutines.delay(2000)
                }
            }
        } finally {
            // 4. Consume the triggers absolutely so it NEVER fights normal swiping
            if (scrollTrigger > 0L) scrollTrigger = 0L
            if (autoScrollUid != null) onAutoScrollComplete()
        }
    }

    LaunchedEffect(activeListState.firstVisibleItemIndex, activeListState.firstVisibleItemScrollOffset) {
        val currentPosition =
            activeListState.firstVisibleItemIndex * 10000 + activeListState.firstVisibleItemScrollOffset
        val isScrollingUp = currentPosition < lastScrollPosition && activeListState.firstVisibleItemIndex > 0
        val isScrollingDown = currentPosition > lastScrollPosition
        lastScrollPosition = currentPosition

        if (activeListState.firstVisibleItemIndex == 0 || isProgrammaticScroll || isScrollingDown) {
            showScrollToTop = false
        } else if (isScrollingUp) {
            showScrollToTop = true
            kotlinx.coroutines.delay(3000)
            if (showScrollToTop && !isProgrammaticScroll) showScrollToTop = false
        }
    }

    BackHandler(enabled = drawerState.isOpen) { scope.launch { drawerState.close() } }

    BackHandler(
        enabled = isSearchActive || searchQuery.isNotBlank() || yankedUid != null || filterTags.isNotEmpty() || filterLocations.isNotEmpty()
    ) {
        when {
            isSearchActive -> isSearchActive = false
            yankedUid != null -> {
                yankedUid = null; yankLockActive = false
            }

            searchQuery.isNotBlank() -> searchQuery = ""
            filterTags.isNotEmpty() || filterLocations.isNotEmpty() -> {
                filterTags = emptySet()
                filterLocations = emptySet()
            }
        }
    }

    val remoteCals = remember(calendars) { calendars.filter { !it.isLocal && !it.isDisabled } }
    val localCals = remember(calendars) { calendars.filter { it.isLocal && !it.isDisabled } }

    if (taskToMove != null) {
        val targetCals =
            remember(calendars) { calendars.filter { it.href != taskToMove!!.calendarHref && !it.isDisabled } }
        AlertDialog(
            onDismissRequest = { taskToMove = null },
            title = { Text(stringResource(R.string.move_task_title)) },
            text = {
                LazyColumn {
                    items(targetCals) { cal ->
                        ListItem(
                            headlineContent = { Text(cal.name) },
                            leadingContent = { NfIcon(NfIcons.CALENDAR, 16.sp) },
                            modifier = Modifier.clickable {
                                scope.launch {
                                    try {
                                        api.moveTask(taskToMove!!.uid, cal.href)
                                        taskToMove = null
                                        updateTaskList()
                                        onDataChanged()
                                    } catch (e: Exception) {
                                        Toast.makeText(
                                            context,
                                            context.getString(R.string.move_failed, e.message ?: ""),
                                            Toast.LENGTH_SHORT
                                        ).show()
                                    }
                                }
                            },
                        )
                    }
                }
            },
            confirmButton = { TextButton(onClick = { taskToMove = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    if (showExportSourceDialog) {
        AlertDialog(
            onDismissRequest = { showExportSourceDialog = false },
            title = { Text(stringResource(R.string.export_select_source_collection)) },
            text = {
                Column {
                    Text(
                        stringResource(R.string.select_local_collection_to_export),
                        fontSize = 14.sp,
                        modifier = Modifier.padding(bottom = 8.dp)
                    )
                    LazyColumn {
                        items(localCals) { cal ->
                            ListItem(
                                headlineContent = { Text(cal.name) },
                                leadingContent = { NfIcon(NfIcons.CALENDAR, 16.sp) },
                                modifier = Modifier.clickable {
                                    exportSourceHref = cal.href
                                    showExportSourceDialog = false
                                    showExportDestDialog = true
                                },
                            )
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    showExportSourceDialog = false
                }) { Text(stringResource(R.string.cancel)) }
            },
        )
    }

    if (showExportDestDialog) {
        AlertDialog(
            onDismissRequest = { showExportDestDialog = false; exportSourceHref = null },
            title = { Text(stringResource(R.string.export_select_destination_collection)) },
            text = {
                Column {
                    exportSourceHref?.let { sourceHref ->
                        val sourceName =
                            localCals.find { it.href == sourceHref }?.name ?: stringResource(R.string.local_label)
                        Text(
                            stringResource(R.string.exporting_from, sourceName),
                            fontSize = 12.sp,
                            modifier = Modifier.padding(bottom = 8.dp)
                        )
                    }
                    Text(
                        stringResource(R.string.select_destination_collection),
                        fontSize = 14.sp,
                        modifier = Modifier.padding(bottom = 8.dp)
                    )
                    LazyColumn {
                        items(remoteCals) { cal ->
                            ListItem(
                                headlineContent = { Text(cal.name) },
                                leadingContent = { NfIcon(NfIcons.CALENDAR, 16.sp) },
                                modifier = Modifier.clickable {
                                    exportSourceHref?.let { sourceHref -> onMigrateLocal(sourceHref, cal.href) }
                                    showExportDestDialog = false
                                    exportSourceHref = null
                                },
                            )
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { showExportDestDialog = false; exportSourceHref = null }) {
                    Text(
                        stringResource(R.string.cancel)
                    )
                }
            },
        )
    }

    // Modal Drawer wrapper covers the entire screen, giving gesture priority edge detection.
    ModalNavigationDrawer(
        drawerState = drawerState,
        drawerContent = {
            ModalDrawerSheet {
                Column(modifier = Modifier.fillMaxHeight().width(300.dp)) {
                    PrimaryTabRow(selectedTabIndex = sidebarTab) {
                        Tab(
                            selected = sidebarTab == 0,
                            onClick = { sidebarTab = 0 },
                            icon = { NfIcon(NfIcons.CALENDARS_VIEW) })
                        Tab(
                            selected = sidebarTab == 1,
                            onClick = { sidebarTab = 1 },
                            icon = { NfIcon(NfIcons.TAGS_VIEW) })
                        Tab(
                            selected = sidebarTab == 2,
                            onClick = { sidebarTab = 2 },
                            icon = { NfIcon(locationTabIcon) })
                    }

                    if (sidebarTab == 1) {
                        val isAllTagsSelected = filterTags.isEmpty()
                        val iconStr = if (isAllTagsSelected) NfIcons.TAG else NfIcons.TAG_OUTLINE

                        Row(
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Row(
                                modifier = Modifier.weight(1f).background(
                                    if (isAllTagsSelected) MaterialTheme.colorScheme.onSurface.copy(alpha = 0.15f) else Color.Transparent,
                                    androidx.compose.foundation.shape.RoundedCornerShape(4.dp)
                                ).clickable { filterTags = emptySet(); scope.launch { drawerState.close() } }
                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                NfIcon(iconStr, size = 14.sp)
                                Spacer(Modifier.width(12.dp))
                                Text(stringResource(R.string.all_tasks), fontSize = 14.sp)
                            }
                            Spacer(Modifier.width(8.dp))
                            Surface(
                                color = if (matchAllCategories) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.secondaryContainer,
                                shape = androidx.compose.foundation.shape.RoundedCornerShape(4.dp),
                                modifier = Modifier.clickable { matchAllCategories = !matchAllCategories }
                            ) {
                                Text(
                                    text = if (matchAllCategories) stringResource(R.string.match_and) else stringResource(
                                        R.string.match_or
                                    ),
                                    fontSize = 12.sp, fontWeight = FontWeight.Bold,
                                    color = if (matchAllCategories) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSecondaryContainer,
                                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                                )
                            }
                        }
                        HorizontalDivider()
                    } else if (sidebarTab == 2) {
                        val isAllLocsSelected = filterLocations.isEmpty()
                        val iconStr = if (isAllLocsSelected) NfIcons.MAP else NfIcons.MAP_O

                        CompactTagRow(
                            name = stringResource(R.string.locations), count = null,
                            color = MaterialTheme.colorScheme.onSurface, isSelected = isAllLocsSelected,
                            icon = iconStr, onClick = { filterLocations = emptySet() },
                            onFocus = { filterLocations = emptySet(); scope.launch { drawerState.close() } }
                        )
                        HorizontalDivider()
                    }

                    LazyColumn(modifier = Modifier.weight(1f), contentPadding = PaddingValues(bottom = 24.dp)) {
                        if (sidebarTab == 0) {
                            item {
                                TextButton(
                                    onClick = {
                                        calendars.forEach {
                                            if (it.href != "local://trash" || defaultCalHref == "local://trash") api.setCalendarVisibility(
                                                it.href,
                                                true
                                            )
                                        }
                                        onDataChanged()
                                        updateTaskList()
                                        pendingTabId = "ALL"
                                        scope.launch { drawerState.close() }
                                    },
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                ) { Text(androidx.compose.ui.res.stringResource(R.string.show_all_collections)) }
                                HorizontalDivider()
                            }
                            items(enabledCals) { cal ->
                                val calColor = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                val isDefault = cal.href == defaultCalHref
                                val iconChar =
                                    if (isDefault) NfIcons.WRITE_TARGET else if (cal.isVisible) NfIcons.VISIBLE else NfIcons.HIDDEN
                                val iconColor = if (isDefault || cal.isVisible) calColor else Color.Gray
                                Row(
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    // Set visibility + Save custom combination
                                    IconButton(
                                        onClick = {
                                            val currentVisible =
                                                enabledCals.filter { it.isVisible }.map { it.href }.toMutableSet()
                                            if (cal.isVisible) currentVisible.remove(cal.href) else currentVisible.add(
                                                cal.href
                                            )

                                            // Correct logic for Custom tab lifecycle
                                            if (currentVisible.size > 1 && currentVisible.size < allHrefs.size) {
                                                customHrefs = currentVisible
                                            } else {
                                                customHrefs = emptySet()
                                            }

                                            api.setCalendarVisibility(cal.href, !cal.isVisible)
                                            onDataChanged()
                                            updateTaskList()
                                        },
                                        enabled = !isDefault
                                    ) { NfIcon(iconChar, color = iconColor) }

                                    // Set write target ONLY (do not close menu, do not jump tab)
                                    TextButton(
                                        onClick = {
                                            api.setDefaultCalendar(cal.href)
                                            localDefaultCalHref = cal.href // Instantly update header

                                            if (!cal.isVisible) {
                                                api.setCalendarVisibility(cal.href, true)
                                                val currentVisible =
                                                    enabledCals.filter { it.isVisible }.map { it.href }.toMutableSet()
                                                currentVisible.add(cal.href)
                                                // Fix 5 is applied here
                                                if (currentVisible.size > 1 && currentVisible.size < allHrefs.size) {
                                                    customHrefs = currentVisible
                                                } else {
                                                    customHrefs = emptySet()
                                                }
                                            }
                                            customWriteTarget = cal.href
                                            if (customHrefs.isNotEmpty() && cal.href !in customHrefs) {
                                                customHrefs = customHrefs + cal.href
                                            }
                                            onDataChanged()
                                        },
                                        modifier = Modifier.weight(1f),
                                        colors = ButtonDefaults.textButtonColors(contentColor = if (isDefault) calColor else MaterialTheme.colorScheme.onSurface)
                                    ) {
                                        Text(cal.name, modifier = Modifier.fillMaxWidth(), textAlign = TextAlign.Start)
                                    }

                                    // Jump to tab (isolate + close menu)
                                    IconButton(onClick = {
                                        scope.launch {
                                            api.isolateCalendar(cal.href)
                                            onDataChanged()
                                            pendingTabId = cal.href
                                            drawerState.close()
                                        }
                                    }) { NfIcon(NfIcons.ARROW_RIGHT, size = 18.sp) }
                                }
                            }
                        } else if (sidebarTab == 1) {
                            items(tags) { tag ->
                                val isUncat = tag.isUncategorized
                                val displayName = if (isUncat) "Uncategorized" else "#${tag.name}"
                                val targetKey = if (isUncat) ":::uncategorized:::" else tag.name
                                val isSelected = filterTags.contains(targetKey)
                                val color = if (isUncat) Color.Gray else getTagColor(tag.name, isDark)
                                val iconStr = if (isSelected) NfIcons.TAG_CHECK else NfIcons.TAG_OUTLINE

                                CompactTagRow(
                                    name = displayName, count = tag.count.toInt(),
                                    color = color, isSelected = isSelected,
                                    icon = iconStr,
                                    onClick = {
                                        filterTags = if (isSelected) filterTags - targetKey else filterTags + targetKey
                                    },
                                    onFocus = { filterTags = setOf(targetKey); scope.launch { drawerState.close() } }
                                )
                            }
                        } else {
                            items(locations) { loc ->
                                val isSelected = filterLocations.contains(loc.name)
                                val iconStr = if (isSelected) NfIcons.CHECK_CIRCLE else NfIcons.MAP_PIN
                                val itemColor = if (isSelected) Color(0xFFFFB300) else Color.Gray

                                CompactTagRow(
                                    name = loc.name, count = loc.count.toInt(),
                                    color = itemColor, isSelected = isSelected,
                                    onClick = {
                                        filterLocations =
                                            if (isSelected) filterLocations - loc.name else filterLocations + loc.name
                                    },
                                    icon = iconStr,
                                    onFocus = {
                                        filterLocations = setOf(loc.name); scope.launch { drawerState.close() }
                                    }
                                )
                            }
                        }
                        item {
                            Box(
                                modifier = Modifier.fillMaxWidth().heightIn(min = 150.dp).padding(vertical = 32.dp),
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
        },
    ) {
        // Wrap everything in a Box to overlay the Edge Detector
        Box(Modifier.fillMaxSize()) {

            val tabsContent: @Composable () -> Unit = {
                AnimatedVisibility(
                    visible = showTabs && tabs.isNotEmpty(),
                    enter = expandVertically(),
                    exit = shrinkVertically()
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(40.dp)
                            .background(MaterialTheme.colorScheme.surface),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        // The leftmost sticky section (All and/or Custom)
                        Row(modifier = Modifier.wrapContentWidth(), verticalAlignment = Alignment.CenterVertically) {
                            val allTabIdx = tabs.indexOfFirst { it.id == "ALL" }
                            if (allTabIdx >= 0) {
                                val isAllSelected = pagerState.currentPage == allTabIdx
                                IconButton(
                                    onClick = { scope.launch { pagerState.animateScrollToPage(allTabIdx) } }
                                ) {
                                    NfIcon(
                                        NfIcons.DATABASE,
                                        size = 18.sp,
                                        color = if (isAllSelected) MaterialTheme.colorScheme.primary else Color.Gray
                                    )
                                }
                            }

                            val customTabIdx = tabs.indexOfFirst { it.id == "CUSTOM" }
                            if (customTabIdx >= 0) {
                                val isCustomSelected = pagerState.currentPage == customTabIdx
                                IconButton(
                                    onClick = { scope.launch { pagerState.animateScrollToPage(customTabIdx) } }
                                ) {
                                    NfIcon(
                                        NfIcons.DATABASE_EYE_OUTLINE,
                                        size = 18.sp,
                                        color = if (isCustomSelected) MaterialTheme.colorScheme.primary else Color.Gray
                                    )
                                }
                            }
                        }

                        // Divider
                        Box(
                            modifier = Modifier.width(1.dp).fillMaxHeight(0.6f)
                                .background(Color.Gray.copy(alpha = 0.3f))
                        )

                        // Scrollable individual calendars
                        val firstScrollableIdx = tabs.indexOfFirst { it.id != "CUSTOM" && it.id != "ALL" }
                        if (firstScrollableIdx >= 0) {
                            ScrollableTabRow(
                                selectedTabIndex = if (pagerState.currentPage >= firstScrollableIdx) pagerState.currentPage - firstScrollableIdx else 0,
                                edgePadding = 0.dp,
                                containerColor = Color.Transparent,
                                modifier = Modifier.weight(1f),
                                divider = {},
                                indicator = { tabPositions ->
                                    val indicatorIndex = (pagerState.currentPage - firstScrollableIdx).coerceIn(
                                        0,
                                        tabPositions.lastIndex
                                    )
                                    TabRowDefaults.PrimaryIndicator(
                                        modifier = Modifier.tabIndicatorOffset(tabPositions[indicatorIndex]),
                                        color = tabs.getOrNull(pagerState.currentPage)?.color ?: activeColor
                                    )
                                }
                            ) {
                                tabs.drop(firstScrollableIdx).forEachIndexed { index, tab ->
                                    val actualPage = index + firstScrollableIdx
                                    val targetColor = tab.color ?: MaterialTheme.colorScheme.onSurface

                                    Tab(
                                        selected = pagerState.currentPage == actualPage,
                                        onClick = { scope.launch { pagerState.animateScrollToPage(actualPage) } },
                                        modifier = Modifier.padding(horizontal = 0.dp),
                                        text = {
                                            Text(
                                                text = tab.name,
                                                color = if (pagerState.currentPage == actualPage) targetColor else Color.Gray,
                                                fontWeight = if (pagerState.currentPage == actualPage) FontWeight.Bold else FontWeight.Normal,
                                                fontSize = 14.sp
                                            )
                                        }
                                    )
                                }
                            }
                        }
                    }
                }
            }

            val headerTitle: @Composable () -> Unit = {
                val currentTab = tabs.getOrNull(pagerState.currentPage)
                val isCustom = currentTab?.id == "CUSTOM"
                val isAll = currentTab?.id == "ALL"

                val writeCalHref = currentTab?.isWriteTarget ?: localDefaultCalHref
                val writeCal = calendars.find { it.href == writeCalHref } ?: calendars.firstOrNull()

                val headerName = writeCal?.name ?: stringResource(R.string.local_label)
                val headerColor = writeCal?.color?.let { parseHexColor(it) } ?: MaterialTheme.colorScheme.onSurface

                val displayName = when {
                    isAll -> "$headerName etc."
                    isCustom -> headerName // Dots appended below
                    else -> currentTab?.name ?: headerName
                }

                val activeCount = remember(tasks, currentTab) {
                    if (currentTab != null) {
                        tasks.count { !it.isDone && it.calendarHref in currentTab.hrefs }
                    } else 0
                }
                val countText = if (tasks.isNotEmpty() && activeCount > 0) "($activeCount)" else ""

                val textMeasurer = rememberTextMeasurer()
                val density = LocalDensity.current

                BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
                    val maxWidth = constraints.maxWidth.toFloat()

                    val textStyle = LocalTextStyle.current.copy(fontSize = 18.sp)
                    val smallTextStyle = LocalTextStyle.current.copy(
                        fontSize = 13.sp,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f)
                    )

                    val nameResult =
                        textMeasurer.measure(text = displayName, style = textStyle.copy(color = headerColor))
                    val countResult = textMeasurer.measure(
                        text = if (countText.isNotEmpty()) " $countText" else "",
                        style = smallTextStyle
                    )
                    val plusResult = textMeasurer.measure(text = "+", style = smallTextStyle)
                    val dotsResult = textMeasurer.measure(text = "...", style = smallTextStyle)

                    val iconSizePx = with(density) { 28.dp.toPx() }
                    val spacerAfterIconPx = with(density) { 8.dp.toPx() }
                    val safetyMarginPx = with(density) { 16.dp.toPx() }

                    val availableForPlus =
                        maxWidth - iconSizePx - spacerAfterIconPx - nameResult.size.width - countResult.size.width - safetyMarginPx
                    val maxVisiblePlus = if (availableForPlus > 0 && plusResult.size.width > 0) {
                        (availableForPlus / plusResult.size.width).toInt()
                    } else 0

                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.clickable {
                            scope.launch {
                                val currentTabId = tabs.getOrNull(pagerState.currentPage)?.id
                                val targetWriteHref = writeCal?.href ?: localDefaultCalHref
                                val writeCalIdx = tabs.indexOfFirst { it.id == targetWriteHref }

                                if (currentTabId == targetWriteHref) {
                                    // Already isolated. Toggle back to Custom or All.
                                    val customIdx = tabs.indexOfFirst { it.id == "CUSTOM" }
                                    val allIdx = tabs.indexOfFirst { it.id == "ALL" }
                                    if (customIdx >= 0) {
                                        pagerState.animateScrollToPage(customIdx)
                                    } else if (allIdx >= 0) {
                                        pagerState.animateScrollToPage(allIdx)
                                    }
                                } else if (writeCalIdx >= 0) {
                                    // On All/Custom. Jump to isolated collection.
                                    pagerState.animateScrollToPage(writeCalIdx)
                                }
                            }
                        }
                    ) {
                        Image(
                            painter = painterResource(id = R.drawable.ic_launcher_foreground),
                            contentDescription = null,
                            modifier = Modifier.size(28.dp)
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(text = displayName, maxLines = 1, overflow = TextOverflow.Ellipsis, color = headerColor)

                        // Render the colored +++ for Custom tab
                        if (isCustom && currentTab != null) {
                            val otherVisible =
                                calendars.filter { it.href in currentTab.hrefs && it.href != writeCal?.href }
                            if (otherVisible.isNotEmpty()) {
                                if (otherVisible.size <= maxVisiblePlus) {
                                    otherVisible.forEach { cal ->
                                        val c = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                        Text(text = "+", color = c, fontSize = 13.sp)
                                    }
                                } else {
                                    val spaceWithDots = availableForPlus - dotsResult.size.width
                                    val fitWithDots = if (spaceWithDots > 0 && plusResult.size.width > 0) {
                                        (spaceWithDots / plusResult.size.width).toInt()
                                    } else 0
                                    val visibleCount = fitWithDots.coerceAtLeast(0)

                                    otherVisible.take(visibleCount).forEach { cal ->
                                        val c = cal.color?.let { parseHexColor(it) } ?: Color.Gray
                                        Text(text = "+", color = c, fontSize = 13.sp)
                                    }
                                    ColoredOverflowDots()
                                }
                            }
                        }

                        if (countText.isNotEmpty()) {
                            Text(
                                text = " $countText",
                                fontSize = 13.sp,
                                color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
                                maxLines = 1
                            )
                        }
                    }
                }
            }

            Scaffold(
                topBar = {
                    Column {
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
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy((-12).dp)
                                ) {
                                    IconButton(onClick = { jumpToRandomTask() }) { NfIcon(currentRandomIcon, 20.sp) }
                                    IconButton(onClick = {
                                        isSearchActive = !isSearchActive
                                        if (!isSearchActive) {
                                            searchQuery = ""; hasRequestedSearchFocus =
                                                false; keyboardController?.hide()
                                        }
                                    }) { NfIcon(if (isSearchActive) NfIcons.SEARCH_STOP else NfIcons.SEARCH, 18.sp) }

                                    if (isLoading || isManualSyncing || activeOpCount > 0 || isPullRefreshing) {
                                        Box(modifier = Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                                            CircularProgressIndicator(
                                                modifier = Modifier.size(24.dp),
                                                strokeWidth = 2.dp
                                            )
                                        }
                                    } else {
                                        val (icon, iconColor) = when {
                                            localHasUnsynced -> Pair(NfIcons.SYNC_ALERT, Color(0xFFEB0000))
                                            lastSyncFailed -> Pair(NfIcons.SYNC_OFF, Color(0xFFFFB300))
                                            else -> Pair(NfIcons.REFRESH, MaterialTheme.colorScheme.onSurface)
                                        }
                                        IconButton(onClick = { handleRefresh() }) {
                                            NfIcon(
                                                icon,
                                                18.sp,
                                                color = iconColor
                                            )
                                        }
                                    }
                                    IconButton(onClick = onSettings) { NfIcon(NfIcons.SETTINGS, 20.sp) }
                                }
                            },
                        )
                        if (isSearchActive) {
                            LaunchedEffect(isSearchActive) {
                                if (!hasRequestedSearchFocus) {
                                    searchFocusRequester.requestFocus(); keyboardController?.show(); hasRequestedSearchFocus =
                                        true
                                }
                            }
                            TextField(
                                value = searchQuery, onValueChange = { searchQuery = it },
                                placeholder = { Text(stringResource(R.string.search_placeholder), fontSize = 14.sp) },
                                singleLine = true, textStyle = LocalTextStyle.current.copy(fontSize = 14.sp),
                                visualTransformation = remember(isDark) {
                                    SmartSyntaxTransformation(
                                        api,
                                        isDark,
                                        true
                                    )
                                },
                                colors = TextFieldDefaults.colors(
                                    focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                    focusedIndicatorColor = Color.Transparent,
                                    unfocusedIndicatorColor = Color.Transparent
                                ),
                                shape = androidx.compose.foundation.shape.RoundedCornerShape(8.dp),
                                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)
                                    .focusRequester(searchFocusRequester),
                            )
                        }
                        if (tabPosition == "top") tabsContent()
                    }
                },
                bottomBar = {
                    Column {
                        if (tabPosition == "bottom") {
                            tabsContent()
                        }
                        if (creatingChildTask != null) {
                            Surface(
                                color = MaterialTheme.colorScheme.tertiaryContainer,
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Row(modifier = Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                    NfIcon(NfIcons.CHILD, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer)
                                    Spacer(Modifier.width(8.dp))
                                    Text(
                                        stringResource(R.string.new_child_of, creatingChildTask.summary),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onTertiaryContainer,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        modifier = Modifier.weight(1f)
                                    )
                                    IconButton(
                                        onClick = { childLockActive = !childLockActive },
                                        modifier = Modifier.size(24.dp)
                                    ) {
                                        NfIcon(
                                            NfIcons.PLUS_LOCK,
                                            16.sp,
                                            if (childLockActive) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onTertiaryContainer.copy(
                                                alpha = 0.5f
                                            )
                                        )
                                    }
                                    Spacer(Modifier.width(4.dp))
                                    IconButton(
                                        onClick = { creatingChildUid = null; childLockActive = false },
                                        modifier = Modifier.size(24.dp)
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
                                        stringResource(R.string.yanked_label) + " " + yankedTask.summary,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        modifier = Modifier.weight(1f)
                                    )
                                    IconButton(
                                        onClick = { yankLockActive = !yankLockActive },
                                        modifier = Modifier.size(24.dp)
                                    ) {
                                        NfIcon(
                                            NfIcons.LINK_LOCK,
                                            16.sp,
                                            if (yankLockActive) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSecondaryContainer.copy(
                                                alpha = 0.5f
                                            )
                                        )
                                    }
                                    Spacer(Modifier.width(4.dp))
                                    IconButton(
                                        onClick = { yankedUid = null; yankLockActive = false },
                                        modifier = Modifier.size(24.dp)
                                    ) { NfIcon(NfIcons.CROSS, 16.sp, MaterialTheme.colorScheme.onSecondaryContainer) }
                                }
                            }
                        }
                        Surface(tonalElevation = 3.dp) {
                            Row(
                                Modifier.padding(16.dp).navigationBarsPadding().imePadding(),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                OutlinedTextField(
                                    value = newTaskText, onValueChange = { newTaskText = it },
                                    placeholder = { Text("${stringResource(R.string.example_buy_cat_food)} !1 @tomorrow #groceries") },
                                    modifier = Modifier.fillMaxWidth(), singleLine = true,
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
                }
            ) { padding ->
                Box(Modifier.padding(padding).fillMaxSize()) {
                    Column(Modifier.fillMaxSize()) {

                        // Export button only visible if currently on the local tab
                        val targetTabForExport = tabs.getOrNull(pagerState.targetPage)
                        val activeIsLocal = targetTabForExport?.id?.startsWith("local://") == true

                        if (activeIsLocal && remoteCals.isNotEmpty()) {
                            FilledTonalButton(
                                onClick = { showExportSourceDialog = true },
                                modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                                colors = ButtonDefaults.filledTonalButtonColors(
                                    containerColor = MaterialTheme.colorScheme.tertiaryContainer,
                                    contentColor = MaterialTheme.colorScheme.onTertiaryContainer
                                ),
                                contentPadding = PaddingValues(vertical = 8.dp),
                            ) {
                                NfIcon(NfIcons.EXPORT, 16.sp, MaterialTheme.colorScheme.onTertiaryContainer)
                                Spacer(Modifier.width(8.dp))
                                Text(stringResource(R.string.export_local_tasks_to_server))
                            }
                        }

                        PullToRefreshBox(
                            isRefreshing = false,
                            onRefresh = { handlePullRefresh() },
                            modifier = Modifier.weight(1f),
                        ) {
                            HorizontalPager(
                                state = pagerState,
                                modifier = Modifier.fillMaxSize(),
                                key = { page -> tabs.getOrNull(page)?.id ?: "ALL_TASKS_$page" }
                            ) { page ->
                                val currentTab = tabs.getOrNull(page)
                                val pageKey = currentTab?.id ?: "ALL_TASKS"
                                val pageListState = listStates.getOrPut(pageKey) { LazyListState() }

                                // Instantaneous list resolution via highly-optimized cache merging
                                val pageTasks = remember(tasks, taskCache, currentTab) {
                                    if (currentTab == null) return@remember emptyList()

                                    val liveTasks = ArrayList<MobileTask>()
                                    val presentHrefs = HashSet<String>()

                                    // 1. Single-pass filter: O(N) time, zero intermediate list allocations
                                    for (task in tasks) {
                                        if (currentTab.hrefs.contains(task.calendarHref)) {
                                            liveTasks.add(task)
                                            presentHrefs.add(task.calendarHref)
                                        }
                                    }

                                    // 2. Fast-path: If all requested calendars are in the live list, return immediately.
                                    // This preserves the Rust backend's perfect interleaved sorting 99% of the time.
                                    if (presentHrefs.size == currentTab.hrefs.size) {
                                        liveTasks
                                    } else {
                                        // 3. Slow-path: User just swiped, backend hasn't fetched the new calendar yet (~50ms window).
                                        // We append the cached tasks to the bottom to prevent a blank screen.
                                        val result = ArrayList<MobileTask>(liveTasks.size + 50)
                                        result.addAll(liveTasks)

                                        val missingHrefs = currentTab.hrefs - presentHrefs
                                        for (href in missingHrefs) {
                                            taskCache[href]?.let { result.addAll(it) }
                                        }
                                        result
                                    }
                                }

                                LazyColumn(
                                    state = pageListState,
                                    contentPadding = PaddingValues(bottom = 80.dp),
                                    modifier = Modifier.fillMaxSize(),
                                ) {
                                    items(pageTasks, key = { it.uid }) { task ->
                                        if (task.virtualType == "none") {
                                            val calColor = calColorMap[task.calendarHref] ?: Color.Gray
                                            val parent = task.parentUid?.let { taskMap[it] }

                                            TaskRow(
                                                task = task,
                                                calColor = calColor,
                                                isDark = isDark,
                                                onToggle = { toggleTask(task) },
                                                onAction = { act -> onTaskAction(act, task) },
                                                onClick = onTaskClick,
                                                yankedUid = yankedUid,
                                                enabledCalendarCount = enabledCalendarCount,
                                                parentCategories = parent?.categories ?: emptyList(),
                                                parentLocation = parent?.location,
                                                aliasMap = aliases,
                                                isHighlighted = task.uid == highlightedUid,
                                                incomingRelations = incomingRelationsMap[task.uid] ?: emptyList()
                                            )
                                        } else {
                                            VirtualTaskRow(task = task) {
                                                val key = task.virtualPayload
                                                expandedGroups =
                                                    if (expandedGroups.contains(key)) expandedGroups - key else expandedGroups + key
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if (showScrollToTop) {
                        FloatingActionButton(
                            onClick = {
                                isProgrammaticScroll = true
                                showScrollToTop = false
                                scope.launch { activeListState.animateScrollToItem(0); isProgrammaticScroll = false }
                            },
                            modifier = Modifier.align(Alignment.BottomEnd).navigationBarsPadding()
                                .offset(x = (-45).dp, y = 40.dp),
                            containerColor = Color.Transparent,
                        ) { NfIcon(scrollToTopIcon, 28.sp, color = Color(0xf2660000)) }
                    }

                    // Global edge swipe detector (now correctly constrained between the bars)
                    if (drawerState.isClosed) {
                        Box(
                            modifier = Modifier
                                .fillMaxHeight()
                                .width(40.dp)
                                .align(Alignment.CenterStart)
                                .pointerInput(Unit) {
                                    awaitEachGesture {
                                        awaitFirstDown(requireUnconsumed = false)
                                        var dragAmount = 0f

                                        while (true) {
                                            val event = awaitPointerEvent(PointerEventPass.Initial)
                                            val change = event.changes.firstOrNull()

                                            if (change == null || !change.pressed) break

                                            dragAmount += (change.position.x - change.previousPosition.x)

                                            if (dragAmount > 24) {
                                                // Swiped right: Open drawer
                                                scope.launch { drawerState.open() }
                                                change.consume()
                                                break
                                            } else if (dragAmount < -12) {
                                                // Swiping left: Yield to Pager
                                                break
                                            }
                                        }
                                    }
                                }
                        )
                    }
                }
            }
        }
    }
}
