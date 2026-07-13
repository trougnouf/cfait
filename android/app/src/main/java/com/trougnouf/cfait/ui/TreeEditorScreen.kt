// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/TreeEditorScreen.kt
package com.trougnouf.cfait.ui

import android.content.ClipData
import android.widget.Toast
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TreeEditorScreen(
    api: CfaitMobile,
    uid: String,
    onBack: () -> Unit,
    onSaveComplete: () -> Unit
) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboard = LocalClipboard.current

    var markdownText by remember { mutableStateOf("") }
    var isLoading by remember { mutableStateOf(true) }
    var isSaving by remember { mutableStateOf(false) }
    val isDark = isSystemInDarkTheme()

    LaunchedEffect(uid) {
        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                markdownText = api.getTaskTreeMarkdown(uid)
            } catch (e: Exception) {
                markdownText = "Error loading tree: ${e.message}"
            } finally {
                isLoading = false
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.edit_tree_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.CROSS, 20.sp) }
                },
                actions = {
                    IconButton(
                        onClick = {
                            scope.launch {
                                clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("tree_markdown", markdownText)))
                                Toast.makeText(context, context.getString(R.string.copied_to_clipboard), Toast.LENGTH_SHORT).show()
                            }
                        },
                        enabled = !isLoading
                    ) {
                        NfIcon(NfIcons.COPY, 20.sp)
                    }
                    IconButton(
                        onClick = {
                            isSaving = true
                            scope.launch {
                                try {
                                    api.syncTaskTreeFromMarkdown(uid, markdownText)
                                    triggerBackgroundSync(context, api)
                                    onSaveComplete()
                                } catch (e: Exception) {
                                    Toast.makeText(context, e.message, Toast.LENGTH_LONG).show()
                                    isSaving = false
                                }
                            }
                        },
                        enabled = !isLoading && !isSaving
                    ) {
                        if (isSaving) {
                            CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                        } else {
                            NfIcon(NfIcons.CHECK, 20.sp, MaterialTheme.colorScheme.primary)
                        }
                    }
                }
            )
        }
    ) { padding ->
        if (isLoading) {
            Box(modifier = Modifier.fillMaxSize().padding(padding), contentAlignment = androidx.compose.ui.Alignment.Center) {
                CircularProgressIndicator()
            }
        } else {
            OutlinedTextField(
                value = markdownText,
                onValueChange = { markdownText = it },
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(16.dp),
                textStyle = androidx.compose.ui.text.TextStyle(fontSize = 14.sp),
                visualTransformation = remember(isDark) {
                    com.trougnouf.cfait.ui.MarkdownTransformation(isDark)
                },
                keyboardOptions = KeyboardOptions.Default.copy(
                    keyboardType = KeyboardType.Text,
                    imeAction = ImeAction.None
                )
            )
        }
    }
}
