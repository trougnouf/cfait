// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.withTransform
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin
import kotlin.random.Random

private const val BURST_SECS = 2.0f
private const val GRAVITY = 2200.0f
private const val PARTICLE_COUNT = 140

private val PALETTE = listOf(
    Color(0xFFF5425C),
    Color(0xFFC2C21F),
    Color(0xFF52C87F),
    Color(0xFF4094E3),
    Color(0xFFB85CD8),
    Color(0xFFFA8CBE),
)

private class Particle(
    val originX: Float,
    val velocityX: Float,
    val velocityY: Float,
    val color: Color,
    val size: Float,
    val spin: Float,
    val phase: Float,
)

/**
 * Full-screen confetti burst celebrating a completed task (per-device
 * `celebrate_completions` setting). Nothing is drawn while [trigger] is 0;
 * each increment starts a new burst on top of the content below.
 */
@Composable
fun ConfettiOverlay(trigger: Int) {
    if (trigger == 0) return

    val particles = remember(trigger) {
        val random = Random(trigger)
        List(PARTICLE_COUNT) { i ->
            val angle = (-90.0f + (random.nextFloat() - 0.5f) * 180.0f) * Math.PI.toFloat() / 180.0f
            val speed = 1200.0f + random.nextFloat() * 2200.0f
            Particle(
                originX = 0.5f + (random.nextFloat() - 0.5f) * 0.3f,
                velocityX = cos(angle) * speed * (if (i % 2 == 0) 1.0f else -1.0f) * abs(sin(angle)),
                velocityY = sin(angle) * speed,
                color = PALETTE[random.nextInt(PALETTE.size)],
                size = 8.0f + random.nextFloat() * 14.0f,
                spin = (random.nextFloat() - 0.5f) * 12.0f,
                phase = random.nextFloat() * (2.0f * Math.PI.toFloat()),
            )
        }
    }
    val progress = remember(trigger) { Animatable(0f) }
    LaunchedEffect(trigger) {
        progress.animateTo(1f, animationSpec = tween(durationMillis = (BURST_SECS * 1000).toInt()))
    }

    Canvas(Modifier.fillMaxSize()) {
        if (progress.value >= 1f) return@Canvas
        val t = progress.value * BURST_SECS
        for (p in particles) {
            val x = p.originX * size.width + p.velocityX * t
            val y = size.height + p.velocityY * t + 0.5f * GRAVITY * t * t
            if (y > size.height + 50 || y < -50) continue
            // Fade out over the last 40% of the burst.
            val alpha = (((BURST_SECS - t) / (BURST_SECS * 0.4f)).coerceIn(0f, 1f))
            // Pseudo-3D flip: the visible width oscillates with the rotation.
            val rotation = p.phase + p.spin * t
            val flip = abs(cos(rotation))
            withTransform({
                rotate(
                    degrees = rotation * 180.0f / Math.PI.toFloat(),
                    pivot = Offset(x, y),
                )
            }) {
                drawRect(
                    color = p.color.copy(alpha = alpha),
                    topLeft = Offset(x - p.size * flip / 2f, y - p.size / 2f),
                    size = Size(p.size * flip, p.size),
                )
            }
        }
    }
}
