// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.ui

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.BuildConfig
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.HelpTab
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HelpScreen(api: CfaitMobile, onBack: () -> Unit) {
    val context = LocalContext.current
    val coroutineScope = rememberCoroutineScope()

    // Fetch categorized data structure from Rust backend
    val helpData = remember { api.getHelpData() }
    val pagerState = rememberPagerState(pageCount = { helpData.size })

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.help_about)) },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                },
            )
        },
    ) { padding ->
        Column(modifier = Modifier.padding(padding).fillMaxSize()) {

            // Swipeable Tab Row for Categories
            ScrollableTabRow(
                selectedTabIndex = pagerState.currentPage,
                edgePadding = 8.dp,
                containerColor = MaterialTheme.colorScheme.surface,
                divider = { HorizontalDivider() }
            ) {
                helpData.forEachIndexed { index, categoryData ->
                    Tab(
                        selected = pagerState.currentPage == index,
                        onClick = { coroutineScope.launch { pagerState.animateScrollToPage(index) } },
                        text = {
                            Text(
                                categoryData.title,
                                fontWeight = if (pagerState.currentPage == index) FontWeight.Bold else FontWeight.Normal
                            )
                        }
                    )
                }
            }

            HorizontalPager(state = pagerState, modifier = Modifier.weight(1f)) { page ->
                val pageData = helpData[page]

                if (pageData.category == HelpTab.ABOUT) {
                    // Custom Kotlin About Screen UI
                    AboutTabContent()
                } else {
                    // Dynamic Rust-backed Documentation
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        items(pageData.sections) { section ->
                            Card(
                                colors = CardDefaults.cardColors(
                                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(
                                        alpha = 0.5f
                                    )
                                ),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Column(Modifier.padding(16.dp)) {
                                    Text(
                                        text = section.title,
                                        fontWeight = FontWeight.Bold,
                                        color = MaterialTheme.colorScheme.primary,
                                        fontSize = 18.sp,
                                        modifier = Modifier.padding(bottom = 12.dp)
                                    )
                                    HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
                                    Spacer(Modifier.height(12.dp))

                                    section.items.forEach { item ->
                                        HelpRow(item)
                                        if (item != section.items.last()) Spacer(Modifier.height(16.dp))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun HelpRow(item: com.trougnouf.cfait.core.MobileHelpItem) {
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .background(MaterialTheme.colorScheme.primaryContainer, RoundedCornerShape(6.dp))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            ) {
                Text(
                    item.keys,
                    fontSize = 14.sp,
                    fontFamily = FontFamily.Monospace,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
            Spacer(Modifier.width(12.dp))
            Text(item.desc, fontSize = 15.sp, modifier = Modifier.weight(1f))
        }
        if (item.example.isNotEmpty()) {
            Text(
                "e.g. ${item.example}",
                fontSize = 13.sp,
                color = Color.Gray,
                fontFamily = FontFamily.Monospace,
                modifier = Modifier.padding(start = 16.dp, top = 4.dp),
            )
        }
    }
}

@Composable
fun AboutTabContent() {
    val context = LocalContext.current
    fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            context.startActivity(intent)
        } catch (e: Exception) {
        }
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        item {
            Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))) {
                Column(Modifier.padding(16.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 12.dp)) {
                        NfIcon(NfIcons.HEART_HAND, 20.sp, Color(0xFFE57373))
                        Spacer(Modifier.width(8.dp))
                        Text(
                            stringResource(R.string.support_card_title),
                            fontWeight = FontWeight.Bold,
                            fontSize = 18.sp
                        )
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
                    Spacer(Modifier.height(8.dp))

                    DonationRow(
                        NfIcons.CREDIT_CARD,
                        "Liberapay",
                        "https://liberapay.com/trougnouf"
                    ) { openUrl("https://liberapay.com/trougnouf") }
                    DonationRow(
                        NfIcons.CREDIT_CARD,
                        "Ko-fi",
                        "https://ko-fi.com/trougnouf"
                    ) { openUrl("https://ko-fi.com/trougnouf") }
                    DonationRow(NfIcons.BANK, "Bank (SEPA)", "BE77 9731 6116 6342", isCopy = true)
                    DonationRow(NfIcons.BITCOIN, "Bitcoin", "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979", isCopy = true)
                }
            }
        }
        item {
            Column(modifier = Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("${stringResource(R.string.app_name)} v${BuildConfig.VERSION_NAME} • GPL3", color = Color.Gray)
                Text("Trougnouf (Benoit Brummer)", color = Color.Gray)
                Text(
                    "https://codeberg.org/trougnouf/cfait",
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable { openUrl("https://codeberg.org/trougnouf/cfait") })
            }
        }
    }
}

@Composable
fun DonationRow(icon: String, name: String, value: String, isCopy: Boolean = false, onClick: () -> Unit = {}) {
    val context = LocalContext.current
    Row(
        modifier = Modifier.fillMaxWidth().clickable {
            if (isCopy) {
                val clipboard =
                    context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                val clip = android.content.ClipData.newPlainText("Donation Address", value)
                clipboard.setPrimaryClip(clip)
                android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
            } else {
                onClick()
            }
        }.padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        NfIcon(icon, 18.sp, Color.Gray)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(name, fontSize = 15.sp)
            Text(value, color = Color.Gray, fontSize = 12.sp, maxLines = 1)
        }
        if (!isCopy) NfIcon(NfIcons.EXTERNAL_LINK, 16.sp, Color.Gray)
        else NfIcon(NfIcons.COPY, 16.sp, Color.Gray)
    }
}
