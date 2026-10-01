//! Parallel iteration where the target has threads, sequential where it does not.
//!
//! `rayon` is a native-only dependency: Cargo.toml already keeps it under
//! `cfg(not(target_arch = "wasm32"))`. The browser build compiles for wasm32,
//! where there is no thread pool, so the fifteen `par_iter` and `into_par_iter`
//! call sites resolve to the sequential stand-in below instead.
//!
//! Everything downstream of those two calls is unchanged: the chains are `map`,
//! `filter`, `enumerate` and `collect`, all of which behave identically on a
//! `std` iterator. The wasm32 build therefore computes the same answer, serially.
//! The hash folds inside the `map` closures were already plain iterator folds,
//! so nothing about the results changes.

#[cfg(not(target_arch = "wasm32"))]
pub use rayon::prelude::*;

/// wasm32 stand-in for the two rayon entry points. Not a general adapter: only
/// the receivers this crate actually uses are implemented.
///
/// The two methods yield different things -- `par_iter` borrows, `into_par_iter`
/// owns -- so they have separate iterator types and no shared item type. The
/// lifetime is on the trait because `par_iter` hands back a borrowing iterator,
/// which an associated type cannot express with `'_`.
#[cfg(target_arch = "wasm32")]
pub trait SequentialParallel<'a> {
    type Iter: Iterator + 'a;
    type IntoIter: Iterator;

    fn par_iter(&'a self) -> Self::Iter;
    fn into_par_iter(self) -> Self::IntoIter;
}

#[cfg(target_arch = "wasm32")]
impl<'a, T: 'a> SequentialParallel<'a> for Vec<T> {
    type Iter = std::slice::Iter<'a, T>;
    type IntoIter = std::vec::IntoIter<T>;

    fn par_iter(&'a self) -> Self::Iter {
        self.iter()
    }
    fn into_par_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

// `Clone` only here, because copying a slice into an iterator needs it; nothing
// on the Vec path does.
#[cfg(target_arch = "wasm32")]
impl<'a, T: Clone + 'a> SequentialParallel<'a> for &'a [T] {
    type Iter = std::slice::Iter<'a, T>;
    type IntoIter = std::vec::IntoIter<T>;

    fn par_iter(&'a self) -> Self::Iter {
        self.iter()
    }
    fn into_par_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

#[cfg(target_arch = "wasm32")]
impl SequentialParallel<'static> for std::ops::Range<usize> {
    type Iter = std::ops::Range<usize>;
    type IntoIter = std::ops::Range<usize>;

    fn par_iter(&'static self) -> Self::Iter {
        self.clone()
    }
    fn into_par_iter(self) -> Self::IntoIter {
        self
    }
}
