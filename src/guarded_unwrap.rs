#[macro_export]
macro_rules! guarded_unwrap {
    (@inner $expr:expr, $none_case:expr) => {
        match GuardedUnwrap::guarded_unwrap_inner($expr) {
            Some(value) => value,
            None => $none_case,
        }
    };

    ($expr:expr, return $label:lifetime $ret:expr) => {
        guarded_unwrap!(@inner $expr, { return $label $ret })
    };

    ($expr:expr, return $label:lifetime) => {
        guarded_unwrap!(@inner $expr, { break $label })
    };

    ($expr:expr, return $ret:expr) => {
        guarded_unwrap!(@inner $expr, { return $ret })
    };

    ($expr:expr, return) => {
        guarded_unwrap!(@inner $expr, { return })
    };

    ($expr:expr, break $label:lifetime $ret:expr) => {
        guarded_unwrap!(@inner $expr, { break $label $ret })
    };

    ($expr:expr, break $label:lifetime) => {
        guarded_unwrap!(@inner $expr, { break $label })
    };

    ($expr:expr, break $ret:expr) => {
        guarded_unwrap!(@inner $expr, { break $ret })
    };

    ($expr:expr, break) => {
        guarded_unwrap!(@inner $expr, { break })
    };

    ($expr:expr, continue $label:lifetime $ret:expr) => {
        guarded_unwrap!(@inner $expr, { continue $label $ret })
    };

    ($expr:expr, continue $label:lifetime) => {
        guarded_unwrap!(@inner $expr, { break $label })
    };

    ($expr:expr, continue $ret:expr) => {
        guarded_unwrap!(@inner $expr, { continue $ret })
    };

    ($expr:expr, continue) => {
        guarded_unwrap!(@inner $expr, { continue })
    };
}

pub trait GuardedUnwrap<T> {
    fn guarded_unwrap_inner(self) -> Option<T>;
}

impl<T> GuardedUnwrap<T> for Option<T> {
    fn guarded_unwrap_inner(self) -> Option<T> {
        self
    }
}

impl<'a, T> GuardedUnwrap<&'a T> for &'a Option<T> {
    fn guarded_unwrap_inner(self) -> Option<&'a T> {
        self.as_ref()
    }
}

impl<T, E> GuardedUnwrap<T> for Result<T, E> {
    fn guarded_unwrap_inner(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(_) => None,
        }
    }
}

impl<'a, T, E> GuardedUnwrap<&'a T> for &'a Result<T, E> {
    fn guarded_unwrap_inner(self) -> Option<&'a T> {
        self.as_ref().ok()
    }
}