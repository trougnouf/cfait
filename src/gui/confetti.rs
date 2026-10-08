// SPDX-License-Identifier: GPL-3.0-or-later
// Confetti burst shown when the user completes a task and the per-device
// `celebrate_completions` setting is enabled. Particle positions are a pure
// function of the elapsed time, so no per-frame physics state is needed.

use iced::widget::canvas::{self, Frame};
use iced::{Color, Point, Rectangle, Size, Vector};

/// How long a burst stays on screen, in seconds.
pub const BURST_SECS: f32 = 2.0;

const PARTICLE_COUNT: usize = 140;
const GRAVITY: f32 = 500.0;
const MIN_SPEED: f32 = 350.0;
const MAX_SPEED: f32 = 900.0;

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

/// Tiny xorshift stream; the burst is purely decorative, so any
/// deterministic-enough spread over `[0, 1)` is fine.
fn rng(seed: u64) -> impl std::iter::Iterator<Item = f32> {
    std::iter::successors(Some(seed), |s| {
        Some(
            s.wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407),
        )
    })
    .map(|s| (s >> 33) as f32 / u32::MAX as f32)
}

impl Confetti {
    pub fn new(window: Size, seed: u64) -> Self {
        let mut rng = rng(seed | 1);
        let origin = Point::new(window.width / 2.0, window.height);
        let particles = (0..PARTICLE_COUNT)
            .map(|i| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + (rng.next().unwrap_or(0.5) - 0.5) * std::f32::consts::PI;
                let speed = MIN_SPEED + rng.next().unwrap_or(0.5) * (MAX_SPEED - MIN_SPEED);
                Particle {
                    origin: Point::new(
                        origin.x + (rng.next().unwrap_or(0.5) - 0.5) * window.width * 0.3,
                        origin.y,
                    ),
                    velocity: Vector::new(angle.cos() * speed, angle.sin() * speed),
                    color: PALETTE[(i
                        + (rng.next().unwrap_or(0.5) * PALETTE.len() as f32) as usize)
                        % PALETTE.len()],
                    size: 4.0 + rng.next().unwrap_or(0.5) * 5.0,
                    spin: (rng.next().unwrap_or(0.5) - 0.5) * 10.0,
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
            let pos = Point::new(
                p.origin.x + p.velocity.x * t,
                p.origin.y + p.velocity.y * t + 0.5 * GRAVITY * t * t,
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
