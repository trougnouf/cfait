// File: android/app/src/main/java/com/cfait/ui/SettingsScreen.kt
package com.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileCalendar
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    api: CfaitMobile,
    onBack: () -> Unit,
    onHelp: () -> Unit,
) {
    var url by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var pass by remember { mutableStateOf("") }
    var insecure by remember { mutableStateOf(false) }
    var hideCompleted by remember { mutableStateOf(false) }
    var sortMonths by remember { mutableStateOf("6") } // New state
    var status by remember { mutableStateOf("") }
    var aliases by remember { mutableStateOf<Map<String, List<String>>>(emptyMap()) }
    var newAliasKey by remember { mutableStateOf("") }
    var newAliasTags by remember { mutableStateOf("") }
    var allCalendars by remember { mutableStateOf<List<MobileCalendar>>(emptyList()) }
    var disabledSet by remember { mutableStateOf<Set<String>>(emptySet()) }

    val scope = rememberCoroutineScope()

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
    }

    LaunchedEffect(Unit) { reload() }

    // Helper to save everything currently in state to disk
    fun saveToDisk() {
        val sortInt = sortMonths.trim().toUIntOrNull()
        api.saveConfig(url, user, pass, insecure, hideCompleted, disabledSet.toList(), sortInt)
    }

    // Connect Button Action
    fun saveAndConnect() {
        scope.launch {
            status = "Connecting..."
            try {
                saveToDisk()
                status = api.connect(url, user, pass, insecure)
                reload()
            } catch (e: Exception) {
                status = "Error: ${e.message}"
            }
        }
    }

    // Navigation Back Action (Saves first)
    fun handleBack() {
        scope.launch {
            saveToDisk()
            onBack()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = { handleBack() }) {
                        NfIcon(NfIcons.BACK, 20.sp)
                    }
                },
                actions = {
                    // Moved Help here as requested
                    IconButton(onClick = onHelp) {
                        NfIcon(NfIcons.HELP, 24.sp)
                    }
                },
            )
        },
    ) { p ->
        LazyColumn(modifier = Modifier.padding(p).padding(16.dp)) {
            // --- SERVER SETTINGS ---
            item {
                Text(
                    "Server Connection",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("CalDAV URL") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = user,
                    onValueChange = { user = it },
                    label = { Text("Username") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(value = pass, onValueChange = {
                    pass = it
                }, label = { Text("Password") }, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth())
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = insecure, onCheckedChange = { insecure = it })
                    Text("Allow insecure SSL")
                }

                Button(onClick = { saveAndConnect() }, modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Text("Save & Connect")
                }
                if (status.isNotEmpty()) {
                    Text(
                        status,
                        color = if (status.startsWith("Error")) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }

                HorizontalDivider(Modifier.padding(vertical = 16.dp))
            }

            // --- PREFERENCES ---
            item {
                Text(
                    "Preferences",
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 8.dp),
                    color = MaterialTheme.colorScheme.primary,
                )

                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = hideCompleted, onCheckedChange = {
                        hideCompleted = it
                        // Immediate save for toggles feels better
                        saveToDisk()
                    })
                    // Rename as requested
                    Text("Hide completed and canceled tasks")
                }

                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Sorting priority cutoff (months):", modifier = Modifier.weight(1f))
                    OutlinedTextField(
                        value = sortMonths,
                        onValueChange = { sortMonths = it },
                        modifier = Modifier.width(80.dp),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        singleLine = true,
                    )
                }
                Text("Tasks due within this range are shown first.", fontSize = 12.sp, color = androidx.compose.ui.graphics.Color.Gray)

                HorizontalDivider(Modifier.padding(vertical = 16.dp))
            }

            // --- CALENDARS ---
            item {
                Text("Manage calendars", fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 8.dp))
            }
            items(allCalendars) { cal ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(
                        checked = !disabledSet.contains(cal.href),
                        onCheckedChange = { enabled ->
                            val newSet = disabledSet.toMutableSet()
                            if (enabled) newSet.remove(cal.href) else newSet.add(cal.href)
                            disabledSet = newSet
                            saveToDisk() // Immediate save
                        },
                    )
                    Text(cal.name)
                }
            }

            // --- ALIASES ---
            item {
                HorizontalDivider(Modifier.padding(vertical = 16.dp))
                Text("Tag aliases", fontWeight = FontWeight.Bold)
            }
            items(aliases.keys.toList()) { key ->
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 4.dp)) {
                    Text("#$key", fontWeight = FontWeight.Bold, modifier = Modifier.width(80.dp))
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
                        label = { Text("Alias") },
                        modifier = Modifier.weight(1f),
                    )
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(value = newAliasTags, onValueChange = {
                        newAliasTags = it
                    }, label = { Text("Tags (comma)") }, modifier = Modifier.weight(1f))
                    IconButton(onClick = {
                        if (newAliasKey.isNotBlank() &&
                            newAliasTags.isNotBlank()
                        ) {
                            val tags = newAliasTags.split(",").map { it.trim().trimStart('#') }.filter { it.isNotEmpty() }
                            scope.launch {
                                api.addAlias(newAliasKey.trimStart('#'), tags)
                                newAliasKey =
                                    ""
                                newAliasTags = ""
                                reload()
                            }
                        }
                    }) { NfIcon(NfIcons.ADD) }
                }
                Spacer(Modifier.height(32.dp))
            }
        }
    }
}
