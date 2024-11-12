pub enum Easing {
    Linear,
    // TODO:
    // CubicBezier,
    // Ease,
    // EaseIn,
    // EaseOut,
    // EaseInOut,
    // Steps(Vec<f32>),
    // Complex(Vec<Easing>),
    // Custom(Box<Fn(f32) -> f32>)
}

impl Easing {
    pub fn point(&self, time_point: f32) -> f32 {
        match self {
            Easing::Linear => time_point,
        }
    }
}
