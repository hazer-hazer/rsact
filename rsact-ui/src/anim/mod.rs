use easing::Easing;
use rsact_reactive::{
    memo::Memo,
    read::ReadSignal,
    signal::{create_signal, Signal},
    write::WriteSignal,
};

pub mod easing;

pub enum AnimIterations {
    Once,
    // TODO
    // N(usize),
    Infinite,
}

pub enum AnimDir {
    /// 0.0 -> 1.0
    Normal,
    /// 1.0 -> 0.0
    Reverse,
    // /// 0.0 -> 1.0
    // Alternate,
    // /// 1.0 -> 0.0 -> 1.0
    // AlternateReverse,
}

// Note: Timestamps in Anim are all relative to start_time, except of source the start_time. So `last_tick = TIME - start_time`
pub struct Anim {
    // value: Signal<f32>,
    last_tick: u32,
    now_millis: Memo<u32>,
    start_time: Option<u32>,
    /// Duration in milliseconds
    duration: u32,
    easing: Easing,
    // dir: AnimDir,
}

impl Anim {
    pub fn value(&self) -> f32 {
        self.value.get()
    }

    // Builder //
    pub(crate) fn new(now_millis: Memo<u32>) -> Self {
        Self {
            value: create_signal(0.0),
            now_millis,
            last_tick: 0,
            start_time: None,
            // TODO: Set to 1.0 for Reverse
            duration: 1000,
            easing: Easing::Linear,
        }
    }

    pub fn duration(mut self, duration: u32) -> Self {
        self.duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    // Note: now_millis can be from other overflow cycle of clock as we use % u32::MAX. Need to set last_tick to difference between start_time and now_millis
    pub(crate) fn tick(&mut self, now_millis: u32) {
        if self.last_tick >= self.duration {
            return;
        }

        if let Some(start_time) = self.start_time {
            self.last_tick =
                (now_millis as i64 - start_time as i64).abs() as u32;

            let time_point =
                (self.last_tick as f32 / self.duration as f32).clamp(0.0, 1.0);
            let value = self.easing.point(time_point);

            self.value.set(value);
        }
    }
}
