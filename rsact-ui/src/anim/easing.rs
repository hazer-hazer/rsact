use core::f32::consts::PI;
use micromath::F32Ext as _;

#[derive(Clone)]
pub enum Easing {
    Linear,
    // TODO:
    // CubicBezier,

    // TODO: Write down what feelings each function gives for user to help pick the right one?
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,

    // Quad (^2)
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,

    // Cubic (^3)
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,

    // Quart (^4)
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,

    // Quint (^5)
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,

    // Circ
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,

    // Elastic
    /// Note: Goes out of 0.0-1.0 range!
    EaseInElastic,
    /// Note: Goes out of 0.0-1.0 range!
    EaseOutElastic,
    /// Note: Goes out of 0.0-1.0 range!
    EaseInOutElastic,

    // Exp
    EaseInExp,
    EaseOutExp,
    EaseInOutExp,

    // Back
    /// Note: Goes out of 0.0-1.0 range!
    EaseInBack,
    /// Note: Goes out of 0.0-1.0 range!
    EaseOutBack,
    /// Note: Goes out of 0.0-1.0 range!
    EaseInOutBack,
    // // Bounce
    // /// Note: Goes out of 0.0-1.0 range!
    // EaseInBounce,
    // /// Note: Goes out of 0.0-1.0 range!
    // EaseOutBounce,
    // /// Note: Goes out of 0.0-1.0 range!
    // EaseInOutBounce,
    // Steps(Vec<f32>),
    // Complex(Vec<Easing>),
    // Custom(Box<Fn(f32) -> f32>)
}

impl Easing {
    // TODO: Possible optimization is LUTs
    /// `f(x) = y` timing function where x is the time point strictly inside 0.0-1.0 range
    pub fn point(&self, x: f32) -> f32 {
        debug_assert!(x >= 0.0 && x <= 1.0);

        const C1: f32 = 1.70158;
        const C2: f32 = C1 * 1.525;
        const C3: f32 = C1 + 1.0;
        const C4: f32 = (2.0 * PI) / 3.0;
        const C5: f32 = (2.0 * PI) / 4.5;

        // Note: Easing functions formulas from [easings.net]

        match self {
            Easing::Linear => x,
            // Cos/Sin
            Easing::EaseInSine => 1.0 - (x * PI / 2.0).cos(),
            Easing::EaseOutSine => 1.0 - (x * PI / 2.0).sin(),
            Easing::EaseInOutSine => -((PI * x).cos() - 1.0) / 2.0,
            // Quad
            Easing::EaseInQuad => x.powi(2),
            Easing::EaseOutQuad => 1.0 - (1.0 - x) * (1.0 - x),
            Easing::EaseInOutQuad => {
                if x < 0.5 {
                    2.0 * x.powi(2)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(2) / 2.0
                }
            },
            // Cubic
            Easing::EaseInCubic => x.powi(3),
            Easing::EaseOutCubic => 1.0 - (1.0 - x).powi(3),
            Easing::EaseInOutCubic => {
                if x < 0.5 {
                    4.0 * x.powi(3)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
                }
            },
            // Quart
            Easing::EaseInQuart => x.powi(4),
            Easing::EaseOutQuart => 1.0 - (1.0 - x).powi(4),
            Easing::EaseInOutQuart => {
                if x < 0.5 {
                    8.0 * x.powi(4)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(4) / 2.0
                }
            },
            // Quint
            Easing::EaseInQuint => x.powi(5),
            Easing::EaseOutQuint => 1.0 - (1.0 - x).powi(5),
            Easing::EaseInOutQuint => {
                if x < 0.5 {
                    16.0 * x.powi(5)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(5) / 2.0
                }
            },
            // Circ
            Easing::EaseInCirc => 1.0 - (1.0 - x.powi(2)).sqrt(),
            Easing::EaseOutCirc => (1.0 - (x - 1.0).powi(2)).sqrt(),
            Easing::EaseInOutCirc => {
                if x < 0.5 {
                    (1.0 - (1.0 - (2.0 * x).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * x + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            },
            // Elastic
            Easing::EaseInElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    -2.0f32.powf(10.0 * x - 10.0)
                        * ((x * 10.0 - 10.75) * C4).sin()
                }
            },
            Easing::EaseOutElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    2.0f32.powf(-10.0 * x) * ((x * 10.0 - 0.75) * C4).sin()
                        + 1.0
                }
            },
            Easing::EaseInOutElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else if x < 0.5 {
                    -(2.0f32.powf(20.0 * x - 10.0)
                        * ((20.0 * x - 11.125) * C5).sin())
                        / 2.0
                } else {
                    (2.0f32.powf(-20.0 * x + 10.0)
                        * ((20.0 * x - 11.125) * C5).sin())
                        / 2.0
                        + 1.0
                }
            },
            // Exp
            Easing::EaseInExp => {
                if x == 0.0 {
                    0.0
                } else {
                    2.0f32.powf(10.0 * x - 10.0)
                }
            },
            Easing::EaseOutExp => {
                if x == 1.0 {
                    1.0
                } else {
                    1.0 - 2.0f32.powf(-10.0 * x)
                }
            },
            Easing::EaseInOutExp => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else if x < 0.5 {
                    2.0f32.powf(20.0 * x - 10.0) / 2.0
                } else {
                    (2.0 - 2.0f32.powf(-20.0 * x + 10.0)) / 2.0
                }
            },
            // Back
            Easing::EaseInBack => C3 * x.powi(3) - C1 * x.powi(2),
            Easing::EaseOutBack => {
                1.0 + C3 * (x - 1.0).powi(3) + C1 * (x - 1.0).powi(2)
            },
            Easing::EaseInOutBack => {
                if x < 0.5 {
                    ((2.0 * x).powi(2) * ((C2 + 1.0) * 2.0 * x - C2)) / 2.0
                } else {
                    ((2.0 * x - 2.0).powi(2)
                        * ((C2 + 1.0) * (x * 2.0 - 2.0) + C2)
                        + 2.0)
                        / 2.0
                }
            },
            // // Bounce
            // Easing::EaseInBounce => {
            //     /*
            //                     function easeInBounce(x: number): number {
            //     return 1 - easeOutBounce(1 - x);
            //     } */
            //     todo!()
            // },
            // Easing::EaseOutBounce => {
            //     /*
            //                     function easeOutBounce(x: number): number {
            //     const n1 = 7.5625;
            //     const d1 = 2.75;

            //     if (x < 1 / d1) {
            //         return n1 * x * x;
            //     } else if (x < 2 / d1) {
            //         return n1 * (x -= 1.5 / d1) * x + 0.75;
            //     } else if (x < 2.5 / d1) {
            //         return n1 * (x -= 2.25 / d1) * x + 0.9375;
            //     } else {
            //         return n1 * (x -= 2.625 / d1) * x + 0.984375;
            //     }
            //     } */
            //     todo!()
            // },
            // Easing::EaseInOutBounce => todo!(),
        }
    }
}
