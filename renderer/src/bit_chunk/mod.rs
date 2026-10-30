pub mod align;
pub mod downsize;
pub mod pack;

macro_rules! unchecked {
    ($src:ident[$idx:expr] $op:tt $rest:expr) => {
        unsafe {
            debug_assert!($idx < $src.len());
            *$src.get_unchecked_mut($idx) $op $rest
        }
    };
    ($src:ident[$idx:expr]) => {
        unsafe {
            debug_assert!($idx < $src.len());
            *$src.get_unchecked($idx)
        }
    };
    (*$ptr:ident[$idx:expr]) => {
        unsafe { *$ptr.add($idx) }
    };
    (*$ptr:ident[$idx:expr] $op:tt $rest:expr) => {
        unsafe { *$ptr.add($idx) $op $rest}
    };
}

pub(super) use unchecked;
