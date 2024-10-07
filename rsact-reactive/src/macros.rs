#[macro_export]
macro_rules! with {
    (|$param: ident $(,)?| $body: expr) => {
        $param.with(|$param| $body)
    };

    (|$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.with(|$param| $crate::macros::with!(|$($rest),+| $body))
    };

    (move |$param: ident $(,)?| $body: expr) => {
        $param.with(move |$param| $body)
    };

    (move |$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.with(move |$param| $crate::macros::with!(move |$($rest),+| $body))
    };
}

pub use with;

// Note: with! macro call inside is intentional to avoid creation of many memos
#[macro_export]
macro_rules! mapped {
    (|$param: ident $(,)?| $body: expr) => {
        $param.mapped(|$param| $body)
    };

    (|$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.mapped(|$param| $crate::macros::with!(|$($rest),+| $body))
    };

    (move |$param: ident $(,)?| $body: expr) => {
        $param.mapped(move |$param| $body)
    };

    (move |$param: ident, $($rest: ident),+ $(,)?| $body: expr) => {
        $param.mapped(move |$param| $crate::macros::with!(move |$($rest),+| $body))
    };
}

pub use mapped;
