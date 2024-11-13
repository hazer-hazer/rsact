use easing::Easing;
use rsact_reactive::{
    memo::Memo,
    read::{ReadSignal, SignalMap},
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

/// This is the state of animation running.
/// - `Stopped`: Animation is not running
/// - `StartRequested`: Denotes that user requested the start but the actual start time is not set yet (see [`Anim::handle`])
/// - `Started`: Animation is started at given time point.
/// These states are needed as an extended Option alternative (with [`AnimRunning::StartRequested`]) to avoid storing current time (`now_millis`) in each animation (even knowing that it is pretty cheap).
enum AnimRunning {
    Stopped,
    StartRequested,
    Started(u32),
    // TODO: `Done` for Infinite repetitions or Stopped is enough?
}

/// The structure that controls the state of animation. It is intended to be stored in Signal and be a dependency for animation value and to give user the API to start/stop/pause, etc. the animation
struct AnimState {
    last_tick: u32,
    running: AnimRunning,
}

impl Default for AnimState {
    fn default() -> Self {
        Self { last_tick: 0, running: AnimRunning::Stopped }
    }
}

/// The actual handle of animation given to user which per can use to control the animation state and get the value. It is a Copy type consisting only of reactive values, so you can move it into closures.
pub struct AnimHandle {
    state: Signal<AnimState>,
    /// Value reactively calculated by [`Anim`] animation parameters depending on current `state`
    value: Memo<f32>,
}

impl AnimHandle {
    pub fn value(&self) -> Memo<f32> {
        self.value
    }

    // TODO: Should start restart the animation if it is already started or do nothing unless it is not?
    pub fn start(&mut self) {
        self.state.update(|state| state.running = AnimRunning::StartRequested)
    }
}

// Note: Timestamps in Anim are all relative to start_time, except of source the start_time. So `last_tick = TIME - start_time`
/// Animation parameters. Not the actual animation user can operate on.
pub struct Anim {
    /// Duration in milliseconds
    duration: u32,
    easing: Easing,
    // dir: AnimDir,
}

impl Anim {
    /// Create new animation, default duration is 1000ms (1sec), easing is Linear
    pub fn new() -> Self {
        Self { duration: 1000, easing: Easing::Linear }
    }

    /// Set animation duration
    pub fn duration(mut self, duration: u32) -> Self {
        self.duration = duration;
        self
    }

    /// Set animation easing function
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    // Note: now_millis can be from other overflow cycle of clock as we use % u32::MAX. Need to set last_tick to difference between start_time and now_millis
    pub(crate) fn handle(self, now_millis: Memo<u32>) -> AnimHandle {
        let mut state = create_signal(AnimState::default());
        let easing = self.easing.clone();
        let duration = self.duration;

        let value = now_millis.map(move |&now_millis| {
            state.update(|state| {
                let start_time = match state.running {
                    AnimRunning::Stopped => {
                        // TODO: Wrong, should return last result. Need `SignalMap::map_memoized` with last returned state
                        return 1.0;
                    },
                    AnimRunning::StartRequested => {
                        state.running = AnimRunning::Started(now_millis);
                        now_millis
                    },
                    AnimRunning::Started(start_time) => start_time,
                };

                // Animation is running here

                if state.last_tick >= duration {
                    // TODO: Not right, 1.0 is not always the end point. Use last state
                    1.0
                } else {
                    state.last_tick =
                        (now_millis as i64 - start_time as i64).abs() as u32;

                    // Note: Clamping to 0.0 is okay for time point as animation always goes from 0.0 to 1.0, even for Reverse (it is on Easing side to calculate value by time point but time point is the same for all easing functions). But Easing result must never be clamped as some of them could return values out of 0.0-1.0 range (for example, some Bezier curves)
                    let time_point = (state.last_tick as f32 / duration as f32)
                        .clamp(0.0, 1.0);

                    easing.point(time_point)
                }
            })
        });

        AnimHandle { state, value }
    }
}
