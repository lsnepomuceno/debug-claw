//! Benchmark mode: scripted input injected through `raw_input_hook`, so
//! the frames go through the same path as real wheel and zoom events.
//! Times come from `Frame::info().cpu_usage` (ui and paint of the
//! previous frame, without waiting for vsync) and from the wall clock
//! between frames.

use std::time::Instant;

use eframe::egui::{self, Event, MouseWheelSource, MouseWheelUnit, Pos2, TouchPhase, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scenario {
    TreeScroll,
    TreeExpand,
    DumpsScroll,
    FlameZoom,
    SourceScroll,
}

impl Scenario {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "tree-scroll" => Self::TreeScroll,
            "tree-expand" => Self::TreeExpand,
            "dumps-scroll" => Self::DumpsScroll,
            "flame-zoom" => Self::FlameZoom,
            "source-scroll" => Self::SourceScroll,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::TreeScroll => "tree-scroll",
            Self::TreeExpand => "tree-expand",
            Self::DumpsScroll => "dumps-scroll",
            Self::FlameZoom => "flame-zoom",
            Self::SourceScroll => "source-scroll",
        }
    }
}

pub struct Bench {
    pub scenario: Scenario,
    pub frames: usize,
    /// Frames to skip before recording (fonts, first layout).
    pub warmup: usize,
    pub frame: usize,
    cpu: Vec<f32>,
    wall: Vec<f32>,
    last: Option<Instant>,
}

impl Bench {
    pub fn new(scenario: Scenario, frames: usize) -> Self {
        Bench { scenario, frames, warmup: 30, frame: 0, cpu: Vec::with_capacity(frames), wall: Vec::with_capacity(frames), last: None }
    }

    pub fn done(&self) -> bool {
        self.frame >= self.warmup + self.frames
    }

    /// Record the previous frame and add this frame's input.
    pub fn tick(&mut self, cpu: Option<f32>, raw: &mut egui::RawInput, at: Pos2) {
        let now = Instant::now();
        if self.frame > self.warmup {
            if let Some(c) = cpu {
                self.cpu.push(c * 1000.0);
            }
            if let Some(l) = self.last {
                self.wall.push(now.duration_since(l).as_secs_f32() * 1000.0);
            }
        }
        self.last = Some(now);
        self.frame += 1;
        if self.frame < 3 {
            raw.events.push(Event::PointerMoved(at));
            return;
        }
        let k = self.frame;
        match self.scenario {
            Scenario::TreeScroll | Scenario::DumpsScroll | Scenario::SourceScroll => {
                // Down for 400 frames, then back up, three lines per frame.
                let dir = if (k / 400) % 2 == 0 { -1.0 } else { 1.0 };
                raw.events.push(wheel(vec2(0.0, 3.0 * dir)));
            }
            Scenario::FlameZoom => {
                // Zoom in 120 frames around a pointer that drifts, then out.
                let x = at.x + ((k as f32) * 0.05).sin() * 200.0;
                raw.events.push(Event::PointerMoved(Pos2::new(x, at.y)));
                let f = if (k / 120) % 2 == 0 { 1.06 } else { 1.0 / 1.06 };
                raw.events.push(Event::Zoom(f));
            }
            Scenario::TreeExpand => {}
        }
    }

    pub fn report(&self, backend: &str, extra: &str) -> String {
        format!(
            "bench={} backend={} frames={} cpu_ms[{}] wall_ms[{}] {extra}",
            self.scenario.name(),
            backend,
            self.cpu.len(),
            stats(&self.cpu),
            stats(&self.wall)
        )
    }
}

fn wheel(delta: egui::Vec2) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta,
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    }
}

fn pct(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i]
}

pub fn stats(v: &[f32]) -> String {
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let mean = if s.is_empty() { f32::NAN } else { s.iter().sum::<f32>() / s.len() as f32 };
    format!(
        "p50={:.2} p95={:.2} p99={:.2} max={:.2} mean={:.2}",
        pct(&s, 0.50),
        pct(&s, 0.95),
        pct(&s, 0.99),
        s.last().copied().unwrap_or(f32::NAN),
        mean
    )
}
