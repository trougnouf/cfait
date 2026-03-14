// Compose UI screen for help documentation.
package com.trougnouf.cfait.ui

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.BuildConfig
import com.trougnouf.cfait.R
import kotlinx.coroutines.launch
import kotlin.random.Random

import com.trougnouf.cfait.core.CfaitMobile

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HelpScreen(api: CfaitMobile, onBack: () -> Unit) {
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    // Resolve localized strings at the composable level so nested functions/coroutines can use them
    val donationLabel = androidx.compose.ui.res.stringResource(R.string.donation_address_label)
    val copiedClipboard = androidx.compose.ui.res.stringResource(R.string.copied_to_clipboard)
    val couldNotOpenUrl = androidx.compose.ui.res.stringResource(R.string.could_not_open_url)

    fun copy(text: String) {
        scope.launch {
            clipboard.setClipEntry(ClipEntry(ClipData.newPlainText(donationLabel, text)))
            Toast.makeText(context, copiedClipboard, Toast.LENGTH_SHORT).show()
        }
    }

    fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            context.startActivity(intent)
        } catch (e: Exception) {
            Toast.makeText(context, couldNotOpenUrl, Toast.LENGTH_SHORT).show()
        }
    }

    // Randomize icon (choose between 3 icons)
    val helpIconRes = when (Random.nextInt(3)) {
        0 -> R.drawable.nf_cod_question_breeze_face_hugs
        1 -> R.drawable.nf_md_robot_confused_breeze_face_hugs
        else -> R.drawable.nf_md_robot_confused_help_breeze_face_hugs
    }

    val apiHelpData = remember { api.getSyntaxHelp() }

    // Pre-resolve localized titles so we can map them back to icons
    val orgTitle = androidx.compose.ui.res.stringResource(R.string.organization)
    val timelineTitle = androidx.compose.ui.res.stringResource(R.string.timeline)
    val recTitle = androidx.compose.ui.res.stringResource(R.string.recurrence)
    val searchTitle = androidx.compose.ui.res.stringResource(R.string.search_and_filtering)

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Image(
                            painter = painterResource(id = helpIconRes),
                            contentDescription = null,
                            modifier = Modifier.width(52.5.dp).height(20.dp)
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(androidx.compose.ui.res.stringResource(R.string.help_about))
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                },
            )
        },
    ) { p ->
        LazyColumn(
            modifier = Modifier.padding(p).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            // Dynamically render sections from Rust API
            apiHelpData.forEach { section ->
                item {
                    val iconStr = when (section.title) {
                        orgTitle -> NfIcons.TAG
                        timelineTitle -> NfIcons.CALENDAR
                        recTitle -> NfIcons.REPEAT
                        searchTitle -> NfIcons.SEARCH
                        else -> NfIcons.INFO
                    }
                    HelpSection(
                        title = section.title,
                        icon = iconStr,
                        items = section.items.map { HelpItem(it.keys, it.desc, it.example) }
                    )
                }
            }

            item {
                Card(
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(
                            alpha = 0.5f
                        )
                    )
                ) {
                    Column(Modifier.padding(12.dp)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.padding(bottom = 8.dp)
                        ) {
                            NfIcon(NfIcons.HEART_HAND, 18.sp, Color(0xFFE57373))
                            Spacer(Modifier.width(8.dp))
                            Text(
                                androidx.compose.ui.res.stringResource(R.string.support_card_title),
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }
                        HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
                        Spacer(Modifier.height(8.dp))

                        DonationRow(
                            icon = NfIcons.CREDIT_CARD,
                            name = "Liberapay",
                            value = "https://liberapay.com/trougnouf",
                            trailingIcon = { NfIcon(NfIcons.EXTERNAL_LINK, 14.sp, Color.Gray) },
                        ) { openUrl("https://liberapay.com/trougnouf") }
                        DonationRow(
                            icon = NfIcons.CREDIT_CARD,
                            name = "Ko-fi",
                            value = "https://ko-fi.com/trougnouf",
                            trailingIcon = { NfIcon(NfIcons.EXTERNAL_LINK, 14.sp, Color.Gray) },
                        ) { openUrl("https://ko-fi.com/trougnouf") }
                        DonationRow(
                            NfIcons.BANK,
                            "Bank (SEPA)",
                            "BE77 9731 6116 6342"
                        ) { copy("BE77 9731 6116 6342") }
                        DonationRow(
                            NfIcons.BITCOIN,
                            "Bitcoin",
                            "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979",
                        ) { copy("bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979") }
                        DonationRow(
                            NfIcons.LITECOIN,
                            "Litecoin",
                            "ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg"
                        ) {
                            copy("ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg")
                        }
                        DonationRow(
                            NfIcons.ETHEREUM,
                            "Ethereum",
                            "0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB"
                        ) {
                            copy("0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB")
                        }
                    }
                }
            }

            item {
                Spacer(Modifier.height(16.dp))
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        "${androidx.compose.ui.res.stringResource(R.string.app_name)} v${BuildConfig.VERSION_NAME} • GPL3",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Gray
                    )
                    Text("Trougnouf (Benoit Brummer)", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                    Text(
                        text = "https://codeberg.org/trougnouf/cfait",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable { openUrl("https://codeberg.org/trougnouf/cfait") },
                    )
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
fun HelpSection(
    title: String,
    icon: String,
    items: List<HelpItem>,
) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
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
                modifier =
                    Modifier
                        .background(MaterialTheme.colorScheme.tertiaryContainer, RoundedCornerShape(4.dp))
                        .padding(horizontal = 6.dp, vertical = 2.dp),
            ) {
                Text(
                    item.syntax,
                    style = MaterialTheme.typography.labelMedium,
                    fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                    color = MaterialTheme.colorScheme.onTertiaryContainer,
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
                modifier = Modifier.padding(start = 0.dp, top = 2.dp),
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
    onClick: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable { onClick() }
                .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
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

data class HelpItem(
    val syntax: String,
    val desc: String,
    val example: String = "",
)
