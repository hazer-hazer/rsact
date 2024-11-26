use alloc::boxed::Box;
use rsact_reactive::memo::Memo;

use super::WidgetStyle;

pub enum CascadingStyle<S: WidgetStyle> {
    // TODO: Maybe require WidgetStyle to impl default and use it here
    Default(S),
    UserDefined(Box<dyn Fn(S, S::Inputs) -> S>),
    Dependent(Memo<S>),
}

impl<S: WidgetStyle> CascadingStyle<S> {
    pub fn new(default: S) -> Self {
        Self::Default(default)
    }

    pub fn user_defined(self, f: impl Fn(S, S::Inputs) -> S) -> Self {
        match self {
            CascadingStyle::Default(default) => todo!(),
            CascadingStyle::UserDefined(_) => todo!(),
            CascadingStyle::Dependent(memo) => todo!(),
        }
    }
}
