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
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.sin
import kotlin.random.Random

private const val BURST_SECS = 2.0f
private const val GRAVITY = 2400.0f
private const val PARTICLE_COUNT = 140
private const val MIN_SPEED = 700.0f
private const val MAX_SPEED = 2600.0f

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
    val drag: Float,
    val color: Color,
    val size: Float,
    val spin: Float,
    val phase: Float,
)

/**
 * Confetti burst celebrating a completed task (per-device
 * `celebrate_completions` setting). Nothing is drawn while [trigger] is 0;
 * each increment starts a new burst on top of the content below. The burst
 * style is randomized per trigger: from a single party-popper point that
 * stays in one region of the screen to a wave spread along the bottom edge.
 */
@Composable
fun ConfettiOverlay(trigger: Int) {
    if (trigger == 0) return

    val particles = remember(trigger) {
        val random = Random(trigger)
        // Launch style: from a single point (party popper) to particles
        // spread along the whole bottom edge. Squared for a bias towards
        // tighter bursts.
        val spread = random.nextFloat().pow(2)
        val burstX = 0.5f + (random.nextFloat() - 0.5f) * (1f - spread)
        List(PARTICLE_COUNT) { i ->
            // Launch anywhere in the upper half-plane, not biased left or right.
            val angle = -random.nextFloat() * Math.PI.toFloat()
            val speed = MIN_SPEED + random.nextFloat() * (MAX_SPEED - MIN_SPEED)
            Particle(
                originX = burstX + (random.nextFloat() - 0.5f) * spread,
                velocityX = cos(angle) * speed,
                velocityY = sin(angle) * speed,
                drag = 0.5f + random.nextFloat() * 3.0f,
                color = PALETTE[(i + random.nextInt(PALETTE.size)) % PALETTE.size],
                size = 6.0f + random.nextFloat() * 12.0f,
                spin = (random.nextFloat() - 0.5f) * 14.0f,
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
            // Closed-form ballistic position under linear air drag:
            // p(t) = p0 + v0 * (1 - e^(-d t))/d + (g/d) * (t - (1 - e^(-d t))/d)
            val decay = 1.0f - exp(-p.drag * t)
            val x = p.originX * size.width + p.velocityX * decay / p.drag
            val y = size.height + p.velocityY * decay / p.drag +
                (GRAVITY / p.drag) * (t - decay / p.drag)
            if (y > size.height + 50 || y < -50 || x < -50 || x > size.width + 50) continue
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
