use easing::Easing;
use rsact_reactive::{
    memo::{Memo, create_memo},
    read::ReadSignal,
    signal::{Signal, create_signal},
    write::WriteSignal,
};

pub mod easing;

#[derive(Clone, Copy, Debug)]
pub enum AnimCycles {
    // TODO: Remove Once being redundant?
    Once,
    N(u32),
    Infinite,
}

impl AnimCycles {
    /// Knowing that animation duration elapsed, should given cycle run?
    fn is_last(&self, cycle: u32) -> bool {
        match self {
            AnimCycles::Once => true,
            &AnimCycles::N(n) => cycle + 1 >= n,
            AnimCycles::Infinite => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AnimDir {
    /// 0.0 -> 1.0
    Normal,
    /// 1.0 -> 0.0
    Reverse,
    /// 0.0 -> 1.0 -> 0.0
    /// Even cycles go in Normal direction, odd cycles go in Reverse direction.
    /// Important: Alternation happens on each cycle, not at the middle of an
    /// animation! [`AnimDir::Alternate`] with [`AnimCycles::Once`] is the same
    /// as [`AnimDir::Normal`]
    Alternate,
    /// 1.0 -> 0.0 -> 1.0
    /// Even cycles go in Reverse direction, odd cycles go in Normal direction.
    /// Important: Alternation happens on each cycle, not at the middle of an
    /// animation! [`AnimDir::AlternateReverse`] with [`AnimCycles::Once`] is
    /// the same as [`AnimDir::Reverse`]
    AlternateReverse,
}

impl AnimDir {
    /// The first value animation should produce.
    fn start_point(&self, cycle: u32) -> f32 {
        // match (self, cycle % 2 == 0) {
        //     // 0.0 -> 1.0 on even iterations
        //     (AnimDir::Normal, false)
        //     // 1.0 -> 0.0 on odd iterations
        //     | (AnimDir::Reverse, true) => 0.0,
        //     // 0.0 -> 1.0 on odd iterations
        //     (AnimDir::Normal, true)
        //     // 1.0 -> 0.0 on even iterations
        //     | (AnimDir::Reverse, false) => 1.0,
        // }

        match (self, cycle % 2 == 1) {
            (AnimDir::Normal, _)
            | (AnimDir::Alternate, false)
            | (AnimDir::AlternateReverse, true) => 0.0,
            (AnimDir::Reverse, _)
            | (AnimDir::AlternateReverse, false)
            | (AnimDir::Alternate, true) => 1.0,
        }
    }

    /// The last value animation should produce. This is dependent to animation
    /// direction
    fn end_point(&self, cycle: u32) -> f32 {
        1.0 - self.start_point(cycle)
    }

    fn time_point(&self, cycle: u32, time_point: f32) -> f32 {
        match (self, cycle % 2 == 1) {
            (AnimDir::Normal, _)
            | (AnimDir::Alternate, false)
            | (AnimDir::AlternateReverse, true) => time_point,
            (AnimDir::Reverse, _)
            | (AnimDir::AlternateReverse, false)
            | (AnimDir::Alternate, true) => 1.0 - time_point,
        }
    }
}

/// This is the state of animation running.
/// - `Done`: Animation is done running. To rerun - call `start` again.
/// - `Ready`: Animation is ready to be started.
/// - `StartRequested`: Denotes that user requested the start but the actual
///   start time is not set yet (see [`Anim::handle`])
/// - `Started`: Animation is started at given time point.
/// These states are needed as an extended Option alternative (with
/// [`AnimStage::StartRequested`]) to avoid storing current time (`now_millis`)
/// in each animation (even knowing that it is pretty cheap).
#[derive(Clone, Copy, Debug)]
enum AnimStage {
    // TODO: Pause
    Done { cycle: u32 },
    Ready,
    StartRequested,
    Running { start_time: u32, cycle: u32 },
}

/// The structure that controls the state of animation. It is intended to be
/// stored in Signal and be a dependency for animation value and to give user
/// the API to start/stop/pause, etc. the animation
struct AnimState {
    /// Relative to start time last value getter timestamp.
    /// Relative means that last_tick = TIME - start_time
    last_tick: u32,
    stage: AnimStage,
}

impl AnimState {
    fn current_cycle(&self) -> u32 {
        match self.stage {
            AnimStage::Done { cycle } => cycle,
            AnimStage::Ready => 0,
            AnimStage::StartRequested => 0,
            AnimStage::Running { cycle, .. } => cycle,
        }
    }
}

impl Default for AnimState {
    fn default() -> Self {
        Self { last_tick: 0, stage: AnimStage::Ready }
    }
}

// TODO: Implement `Into<MaybeReactive<T>>` for different types to pass
// animation right into Widget parameters. Also for RangeU8

// TODO: Rewrite animations to fixed-point math with, for example, u32 range?

/// The actual handle of animation given to user which per can use to control
/// the animation state and get the value. It is a Copy type consisting only of
/// reactive values, so you can move it into closures.
pub struct AnimHandle {
    state: Signal<AnimState>,
    /// Value reactively calculated by [`Anim`] animation parameters depending
    /// on current `state`
    pub value: Memo<f32>,
}

impl AnimHandle {
    // pub fn value(&self) -> Memo<f32> {
    //     self.value
    // }

    // TODO: Should `start` restart the animation if it is already started or do
    // nothing unless it is not?
    //  - I think should restart.
    /// Start the animation. Restarts already running animation.
    pub fn start(&mut self) {
        self.state
            .update(|state| state.stage = AnimStage::StartRequested)
    }

    /// Stop the animation, resetting the state. The value will give the latest
    /// result
    pub fn stop(&mut self) {
        self.state.update(|state| {
            state.stage = AnimStage::Done { cycle: state.current_cycle() }
        })
    }

    // TODO: `pause` is not the best idea because of now_millis wrapping. The
    // moment user paused animation may be from the other cycle of now_millis,
    // but same for all timing, so I need to figure out how to fix this. pub
    // fn pause(&mut self) {}
}

// Note: Timestamps in Anim are all relative to start_time, except of source the
// start_time. So `last_tick = TIME - start_time`
/// Animation parameters. Not the actual animation user can operate on.
/// Mind that full animation duration is given by delay + duration.
pub struct Anim {
    /// Duration in milliseconds
    duration: u32,
    easing: Easing,
    direction: AnimDir,
    // TODO: Negative delays as in CSS?
    // TODO: Delay before each iteration? Not as in CSS :)
    /// Animation will start after specified delay.
    delay: u32,
    /// Count of cycles animation will repeat.
    cycles: AnimCycles,
}

impl Anim {
    /// Create new animation, default duration is 1000ms (1sec), easing is
    /// Linear
    pub fn new() -> Self {
        Self {
            duration: 1000,
            easing: Easing::Linear,
            direction: AnimDir::Normal,
            delay: 0,
            cycles: AnimCycles::Once,
        }
    }

    /// Set animation duration in milliseconds
    pub fn duration(mut self, duration_ms: u32) -> Self {
        self.duration = duration_ms;
        self
    }

    /// Set animation easing function
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn direction(mut self, direction: AnimDir) -> Self {
        self.direction = direction;
        self
    }

    /// Set animation delay in milliseconds
    pub fn delay(mut self, delay_ms: u32) -> Self {
        self.delay = delay_ms;
        self
    }

    pub fn cycles(mut self, cycles: u32) -> Self {
        self.cycles = AnimCycles::N(cycles);
        self
    }

    pub fn infinite(mut self) -> Self {
        self.cycles = AnimCycles::Infinite;
        self
    }

    // TODO: Can fixed framerate increase performance significantly if anything
    // we'll gather from it is avoiding easing computations?
    // Note: now_millis can be from other overflow cycle of clock as we use %
    // u32::MAX. Need to set last_tick to difference between start_time and
    // now_millis
    pub(crate) fn handle(self, now_millis: Memo<u32>) -> AnimHandle {
        let mut state = create_signal(AnimState::default());
        let easing = self.easing.clone();
        let duration = self.duration;
        let delay = self.delay;
        let dir = self.direction;
        let cycles = self.cycles;

        let value = create_memo(move || {
            // Note: If animation is not running (or start is not requested),
            // don't depend on now_millis, so animation value code won't rerun
            // on any now_millis change.
            match state.with(|state| state.stage) {
                AnimStage::Ready => return dir.start_point(0),
                AnimStage::Done { cycle } => return dir.end_point(cycle),
                _ => {},
            }

            // A zero-cycle animation never runs: report the start value and
            // never depend on the clock. Without this guard `is_last` treats
            // cycle 0 as the last cycle, so `N(0)` played one full cycle.
            if matches!(cycles, AnimCycles::N(0)) {
                return dir.start_point(0);
            }

            let now_millis = now_millis.get();

            // Note: We don't need to notify about state changes. When state is
            // changed, next `value` memo call will check if it is changed. If
            // `update` was used, we'd recursively call `value` memo and ran
            // into borrowing error.
            let value = state.update_untracked(|state| {
                let (mut start_time, cycle) = match state.stage {
                    AnimStage::Done { .. } | AnimStage::Ready => unreachable!(),
                    AnimStage::StartRequested => {
                        // Reset the relative clock so a restart doesn't inherit
                        // the previous run's `last_tick` (which, being >=
                        // duration after completion, would make the completion
                        // check below fire immediately and no-op the restart).
                        state.last_tick = 0;
                        state.stage = AnimStage::Running {
                            start_time: now_millis,
                            cycle: 0,
                        };
                        (now_millis, 0)
                    },
                    AnimStage::Running { start_time, cycle } => {
                        (start_time, cycle)
                    },
                };

                // Animation is running (or delaying) here //

                // Set delay only for first cycle. Can be extended with
                // per-cycle delays
                let delay = if cycle == 0 { delay } else { 0 };

                // extern crate std;
                // std::println!(
                //     "Anim tick stage: {:?}. Cycle: {cycle}",
                //     state.stage
                // );

                let value = if state.last_tick >= duration + delay {
                    if cycles.is_last(cycle) {
                        state.stage = AnimStage::Done { cycle };
                        dir.end_point(cycle)
                    } else {
                        let cycle = cycle + 1;
                        state.stage = AnimStage::Running {
                            start_time: now_millis,
                            cycle,
                        };
                        start_time = now_millis;
                        dir.start_point(cycle)
                    }
                } else if state.last_tick < delay {
                    dir.start_point(cycle)
                } else {
                    // Note: Clamping to 0.0 is okay for time point as animation
                    // always goes from 0.0 to 1.0, even for Reverse (it is on
                    // Easing side to calculate value by time point but time
                    // point is the same for all easing functions). Easing
                    // result must never be clamped as some of them could return
                    // values out of 0.0-1.0 range (for example, some Bezier
                    // curves), but the start point and end point are always 0.0
                    // or 1.0.
                    let time_point = ((state.last_tick as f32 - delay as f32)
                        / duration as f32)
                        .clamp(0.0, 1.0);

                    let time_point = dir.time_point(cycle, time_point);

                    easing.point(time_point)
                };

                // `wrapping_sub` yields the correct elapsed time across a
                // single u32 clock wrap. The previous `.abs()` of the signed
                // difference exploded to a near-`u32::MAX` value whenever
                // `now_millis` wrapped past `start_time`, instantly completing
                // any running animation.
                state.last_tick = now_millis.wrapping_sub(start_time);

                value
            });

            value
        });

        AnimHandle { state, value }
    }
}

#[cfg(test)]
mod tests {
    use super::{Anim, AnimHandle, AnimStage, easing::Easing};
    use rsact_reactive::prelude::*;
    use rsact_reactive::runtime::with_new_runtime;

    /// Build an animation handle driven by a manually-advanced clock signal.
    /// Set the clock, then read `handle.value` to advance the animation.
    fn harness(anim: Anim) -> (Signal<u32>, AnimHandle) {
        let clock = create_signal(0u32);
        let now = create_memo(move || clock.get());
        (clock, anim.handle(now))
    }

    // b.2(a): a completed animation must restart on `start()`. The bug: the
    // completion check reads a stale `last_tick` that neither `start()` nor the
    // StartRequested->Running transition resets, so the restarted animation
    // instantly re-completes (no-op) every other `start()`.
    #[test]
    fn restart_after_completion_runs_again() {
        with_new_runtime(|_| {
            let (mut clock, mut h) = harness(Anim::new().duration(100));

            // Run the first cycle to completion.
            h.start();
            clock.set(0);
            h.value.get();
            clock.set(200);
            h.value.get();
            clock.set(400);
            h.value.get();
            assert!(
                matches!(h.state.with(|s| s.stage), AnimStage::Done { .. }),
                "first run should have completed"
            );

            // Restart: after a mid-duration tick the animation must be Running,
            // not have jumped straight back to Done.
            h.start();
            clock.set(500);
            h.value.get();
            clock.set(550);
            h.value.get();
            let stage = h.state.with(|s| s.stage);
            assert!(
                matches!(stage, AnimStage::Running { .. }),
                "restart silently no-opped: stage={stage:?}"
            );
        });
    }

    // b.2(b): a running animation must survive a u32 clock wrap. The bug:
    // elapsed time is computed as `(now - start_time).abs()`, which explodes to
    // a near-u32::MAX value when the clock wraps, instantly completing the
    // animation. `wrapping_sub` yields the correct small delta.
    #[test]
    fn running_animation_survives_clock_wrap() {
        with_new_runtime(|_| {
            let (mut clock, mut h) = harness(Anim::new().duration(1000));
            let base = u32::MAX - 50; // start just before the wrap

            h.start();
            clock.set(base);
            h.value.get();
            clock.set(base.wrapping_add(60));
            h.value.get();
            clock.set(base.wrapping_add(120)); // ~120ms elapsed, well under 1000
            let v = h.value.get();

            assert!(
                matches!(h.state.with(|s| s.stage), AnimStage::Running { .. }),
                "clock wrap prematurely completed the animation"
            );
            assert!(
                v < 0.5,
                "clock wrap corrupted elapsed time: value jumped to {v}"
            );
        });
    }

    // b.2(c): `AnimCycles::N(0)` means zero cycles and must not animate. The
    // bug: `is_last` treats cycle 0 as the last cycle, so N(0) plays one full
    // cycle (0.0 -> 1.0) instead of nothing.
    #[test]
    fn zero_cycles_plays_nothing() {
        with_new_runtime(|_| {
            let (mut clock, mut h) =
                harness(Anim::new().duration(100).cycles(0));

            h.start();
            clock.set(0);
            h.value.get();
            clock.set(50);
            h.value.get();
            clock.set(300); // well past one cycle
            let v = h.value.get();

            assert_eq!(
                v, 0.0,
                "N(0) must play zero cycles, but it animated to {v}"
            );
        });
    }

    // b.2(d): `EaseOutSine` must run 0 -> 1 (easings.net: sin(x*pi/2)). The bug
    // inverted it to `1 - sin(x*pi/2)`, running 1 -> 0.
    #[test]
    fn ease_out_sine_runs_from_zero_to_one() {
        assert!(
            Easing::EaseOutSine.point(0.0).abs() < 1e-6,
            "EaseOutSine must start at 0, got {}",
            Easing::EaseOutSine.point(0.0)
        );
        assert!(
            (Easing::EaseOutSine.point(1.0) - 1.0).abs() < 1e-6,
            "EaseOutSine must end at 1, got {}",
            Easing::EaseOutSine.point(1.0)
        );
        // Ease-out is concave: the midpoint sits above the linear diagonal.
        assert!(
            Easing::EaseOutSine.point(0.5) > 0.5,
            "EaseOutSine must be an ease-out curve"
        );
    }
}
