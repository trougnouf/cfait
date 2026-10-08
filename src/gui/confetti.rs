// SPDX-License-Identifier: GPL-3.0-or-later
// Confetti burst shown when the user completes a task and the per-device
// `celebrate_completions` setting is enabled. Particle positions are a pure
// function of the elapsed time, so no per-frame physics state is needed.

use iced::widget::canvas::{self, Frame};
use iced::{Color, Point, Rectangle, Size, Vector};

/// How long a burst stays on screen, in seconds.
pub const BURST_SECS: f32 = 2.0;

const PARTICLE_COUNT: usize = 140;
const GRAVITY: f32 = 800.0;
const MIN_SPEED: f32 = 250.0;
const MAX_SPEED: f32 = 1300.0;

const PALETTE: [Color; 6] = [
    Color::from_rgb(0.96, 0.26, 0.36),
    Color::from_rgb(1.0, 0.76, 0.25),
    Color::from_rgb(0.32, 0.78, 0.42),
    Color::from_rgb(0.25, 0.58, 0.95),
    Color::from_rgb(0.72, 0.36, 0.88),
    Color::from_rgb(0.98, 0.55, 0.75),
];

#[derive(Debug, Clone)]
struct Particle {
    origin: Point,
    velocity: Vector,
    /// Linear air-drag coefficient (per second). Varies the flight arcs from
    /// snappy to floaty.
    drag: f32,
    color: Color,
    size: f32,
    spin: f32,
    phase: f32,
}

/// An active burst. Spawned on task completion, cleared once older than
/// `BURST_SECS` (see `Message::ConfettiTick`).
#[derive(Debug, Clone)]
pub struct Confetti {
    start: std::time::Instant,
    particles: Vec<Particle>,
}

/// splitmix64 finalizer: full avalanche, so any seed difference produces a
/// completely different burst.
pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// splitmix64 stream over `[0, 1)`.
fn rng(seed: u64) -> impl std::iter::Iterator<Item = f32> {
    std::iter::successors(Some(seed), |s| Some(s.wrapping_add(0x9E37_79B9_7F4A_7C15)))
        .map(mix64)
        .map(|s| (s >> 40) as f32 / (1u64 << 24) as f32)
}

impl Confetti {
    pub fn new(window: Size, seed: u64) -> Self {
        let mut rng = rng(seed);
        let origin = Point::new(window.width / 2.0, window.height);
        // Per-burst launch style: from a single point (party popper) to
        // particles spread along the whole bottom edge. Squared for a bias
        // towards tighter bursts.
        let spread = rng.next().unwrap_or(0.5).powi(2) * window.width;
        let burst_x = origin.x + (rng.next().unwrap_or(0.5) - 0.5) * (window.width - spread);
        let particles = (0..PARTICLE_COUNT)
            .map(|i| {
                // Launch anywhere in the upper half-plane, not biased left or
                // right.
                let angle = -rng.next().unwrap_or(0.5) * std::f32::consts::PI;
                let speed = MIN_SPEED + rng.next().unwrap_or(0.5) * (MAX_SPEED - MIN_SPEED);
                Particle {
                    origin: Point::new(
                        burst_x + (rng.next().unwrap_or(0.5) - 0.5) * spread,
                        origin.y,
                    ),
                    velocity: Vector::new(angle.cos() * speed, angle.sin() * speed),
                    drag: 0.5 + rng.next().unwrap_or(0.5) * 3.0,
                    color: PALETTE[(i
                        + (rng.next().unwrap_or(0.5) * PALETTE.len() as f32) as usize)
                        % PALETTE.len()],
                    size: 3.0 + rng.next().unwrap_or(0.5) * 9.0,
                    spin: (rng.next().unwrap_or(0.5) - 0.5) * 14.0,
                    phase: rng.next().unwrap_or(0.5) * std::f32::consts::TAU,
                }
            })
            .collect();
        Self {
            start: std::time::Instant::now(),
            particles,
        }
    }

    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed() >= BURST_SECS
    }
}

/// Canvas program rendering an active [`Confetti`] burst on top of the app.
/// `update` is left at its default (no input handling), so clicks pass
/// through the overlay to the widgets underneath.
pub struct ConfettiProgram {
    pub confetti: Confetti,
}

impl canvas::Program<crate::gui::Message> for ConfettiProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let t = self.confetti.elapsed();
        let mut frame = Frame::with_bounds(renderer, bounds);

        for p in &self.confetti.particles {
            // Closed-form ballistic position under linear air drag:
            // p(t) = p0 + v0 * (1 - e^(-d t))/d + (g/d) * (t - (1 - e^(-d t))/d)
            let decay = 1.0 - (-p.drag * t).exp();
            let pos = Point::new(
                p.origin.x + p.velocity.x * decay / p.drag,
                p.origin.y
                    + p.velocity.y * decay / p.drag
                    + (GRAVITY / p.drag) * (t - decay / p.drag),
            );
            // Fade out over the last 40% of the burst.
            let alpha = ((BURST_SECS - t) / (BURST_SECS * 0.4)).clamp(0.0, 1.0);
            let mut color = p.color;
            color.a = alpha;
            // Pseudo-3D flip: the visible width oscillates with the rotation.
            let flip = (p.phase + p.spin * t).cos().abs();
            let rotation = p.phase + p.spin * t;

            frame.with_save(|frame| {
                frame.translate(Vector::new(pos.x, pos.y));
                frame.rotate(rotation);
                frame.fill_rectangle(
                    Point::new(-p.size * flip / 2.0, -p.size / 2.0),
                    Size::new(p.size * flip, p.size),
                    color,
                );
            });
        }

        vec![frame.into_geometry()]
    }
}
