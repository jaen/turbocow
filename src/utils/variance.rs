// When nightly feature is available, re-export from core
#[cfg(feature = "nightly-phantom-variance")]
pub use core::marker::PhantomCovariantLifetime;

// Polyfill for stable
#[cfg(not(feature = "nightly-phantom-variance"))]
mod polyfill {
    use core::marker::PhantomData;

    /// Zero-sized type used to mark a lifetime as covariant.
    ///
    /// This is a polyfill for [`core::marker::PhantomCovariantLifetime`][2]
    /// (nightly `phantom_variance_markers`, tracking issue #135806).
    ///
    /// Covariant lifetimes must live at least as long as declared.
    /// See [the reference][1] for more information.
    ///
    /// [1]: https://doc.rust-lang.org/stable/reference/subtyping.html#variance
    // A URL rather than an intra-doc link: the item only exists on nightly, so
    // the path does not resolve on the MSRV / stable toolchains.
    /// [2]: https://doc.rust-lang.org/nightly/core/marker/struct.PhantomCovariantLifetime.html
    ///
    /// ## Layout
    ///
    /// For all `'a`, the following are guaranteed:
    /// * `size_of::<PhantomCovariantLifetime<'a>>() == 0`
    /// * `align_of::<PhantomCovariantLifetime<'a>>() == 1`
    #[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(transparent)]
    pub struct PhantomCovariantLifetime<'a>(PhantomData<fn() -> &'a ()>);

    impl PhantomCovariantLifetime<'_> {
        /// Constructs a new instance of the variance marker.
        pub const fn new() -> Self {
            Self(PhantomData)
        }
    }

    impl core::fmt::Debug for PhantomCovariantLifetime<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("PhantomCovariantLifetime")
        }
    }
}

#[cfg(not(feature = "nightly-phantom-variance"))]
pub use polyfill::PhantomCovariantLifetime;
