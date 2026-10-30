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
    (&mut $src:ident[$st:tt..$en:expr]) => {
        unsafe {
            #[allow(unused_comparisons)]
            {
                debug_assert!($st <= $en);
                debug_assert!($en <= $src.len());
            }
            $src.get_unchecked_mut($st..$en)
        }
    };
    (&$src:ident[$st:tt..$en:expr]) => {
        unsafe {
            #[allow(unused_comparisons)]
            {
                debug_assert!($st <= $en);
                debug_assert!($en <= $src.len());
            }
            $src.get_unchecked($st..$en)
        }
    };
    (&$src:ident[..$en:expr])     => { unchecked!(&$src[0..$en]) };
    (&mut $src:ident[..$en:expr]) => { unchecked!(&mut $src[0..$en]) };
}

pub(super) use unchecked;
