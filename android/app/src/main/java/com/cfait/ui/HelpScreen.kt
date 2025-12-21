// File: ./android/app/src/main/java/com/cfait/ui/HelpScreen.kt
package com.cfait.ui

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.BuildConfig
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HelpScreen(onBack: () -> Unit) {
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    fun copy(text: String) {
        scope.launch {
            clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("Donation Address", text)))
            Toast.makeText(context, "Copied to clipboard", Toast.LENGTH_SHORT).show()
        }
    }

    fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            context.startActivity(intent)
        } catch (e: Exception) {
            Toast.makeText(context, "Could not open URL", Toast.LENGTH_SHORT).show()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Help & About") },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                }
            )
        }
    ) { p ->
        LazyColumn(
            modifier = Modifier.padding(p).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            item {
                HelpSection("Organization", NfIcons.TAG, listOf(
                    HelpItem("!1", "Priority high (1) to low (9)", "!1, !5, !9"),
                    HelpItem("#tag", "Add category. Use ':' for sub-tags.", "#work, #dev:backend"),
                    HelpItem("#a=#b,#c", "Define/update alias inline.", "#groceries=#home,#shopping"),
                    HelpItem("~30m", "Estimated duration (m/h/d/w).", "~30m, ~1.5h, ~2d")
                ))
            }

            item {
                HelpSection("Timeline", NfIcons.CALENDAR, listOf(
                    HelpItem("@date", "Due date. Deadline.", "@tomorrow, @2025-12-31"),
                    HelpItem("^date", "Start date. Hides until date.", "^next week, ^2025-01-01"),
                    HelpItem("Offsets", "Add time from today.", "1d, 2w, 3mo, 4y"),
                    HelpItem("@in", "Natural relative offset.", "@in 3 days, ^in 2 weeks"),
                    HelpItem("Keywords", "Relative dates supported.", "today, tomorrow, next week")
                ))
            }

            item {
                HelpSection("Recurrence", NfIcons.REPEAT, listOf(
                    HelpItem("@daily", "Quick presets.", "@daily, @weekly, @monthly"),
                    HelpItem("@every X", "Custom intervals.", "@every 3 days, @every 2 weeks")
                ))
            }

            item {
                HelpSection("Search & Filtering", NfIcons.SEARCH, listOf(
                    HelpItem("text", "Matches title/desc.", "buy cat food"),
                    HelpItem("is:state", "Filter by state.", "is:done, is:active"),
                    HelpItem("Operators", "Compare (<, >, <=, >=).", "~<20m, !<3 (high prio)"),
                    HelpItem("Dates", "Filter timeframe.", "@<today (Overdue), ^>tomorrow")
                ))
            }

            item {
                Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))) {
                    Column(Modifier.padding(12.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                            NfIcon(NfIcons.HEART_HAND, 18.sp, Color(0xFFE57373))
                            Spacer(Modifier.width(8.dp))
                            Text("Support Development", fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.onSurface)
                        }
                        HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
                        Spacer(Modifier.height(8.dp))
                        
                        DonationRow(
                            icon = NfIcons.CREDIT_CARD,
                            name = "Liberapay",
                            value = "https://liberapay.com/trougnouf",
                            trailingIcon = { NfIcon(NfIcons.EXTERNAL_LINK, 14.sp, Color.Gray) }
                        ) { openUrl("https://liberapay.com/trougnouf") }
                        DonationRow(NfIcons.BANK, "Bank (SEPA)", "BE77 9731 6116 6342") { copy("BE77 9731 6116 6342") }
                        DonationRow(NfIcons.BITCOIN, "Bitcoin", "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979") { copy("bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979") }
                        DonationRow(NfIcons.LITECOIN, "Litecoin", "ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg") { copy("ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg") }
                        DonationRow(NfIcons.ETHEREUM, "Ethereum", "0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB") { copy("0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB") }
                    }
                }
            }

            item {
                Spacer(Modifier.height(16.dp))
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text("Cfait v${BuildConfig.VERSION_NAME} • GPL3", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                    Text("Trougnouf (Benoit Brummer)", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                    Text(
                        text = "https://codeberg.org/trougnouf/cfait",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable { openUrl("https://codeberg.org/trougnouf/cfait") }
                    )
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
fun HelpSection(title: String, icon: String, items: List<HelpItem>) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
    ) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                NfIcon(icon, 18.sp, MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(8.dp))
                Text(title, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
            Spacer(Modifier.height(8.dp))
            items.forEach { item ->
                HelpRow(item)
                if (item != items.last()) Spacer(Modifier.height(8.dp))
            }
        }
    }
}

@Composable
fun HelpRow(item: HelpItem) {
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .background(MaterialTheme.colorScheme.tertiaryContainer, RoundedCornerShape(4.dp))
                    .padding(horizontal = 6.dp, vertical = 2.dp)
            ) {
                Text(
                    item.syntax, 
                    style = MaterialTheme.typography.labelMedium, 
                    fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                    color = MaterialTheme.colorScheme.onTertiaryContainer
                )
            }
            Spacer(Modifier.width(8.dp))
            Text(item.desc, style = MaterialTheme.typography.bodySmall, modifier = Modifier.weight(1f))
        }
        if (item.example.isNotEmpty()) {
            Text(
                "e.g. ${item.example}", 
                style = MaterialTheme.typography.bodySmall, 
                color = Color.Gray, 
                fontSize = 10.sp,
                modifier = Modifier.padding(start = 0.dp, top = 2.dp)
            )
        }
    }
}

@Composable
fun DonationRow(
    icon: String,
    name: String,
    value: String,
    trailingIcon: @Composable () -> Unit = { NfIcon(NfIcons.COPY, 14.sp, Color.Gray) },
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        NfIcon(icon, 16.sp, Color.Gray)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurface)
            Text(value, style = MaterialTheme.typography.bodySmall, color = Color.Gray, fontSize = 10.sp, maxLines = 1)
        }
        trailingIcon()
    }
}

data class HelpItem(val syntax: String, val desc: String, val example: String = "")