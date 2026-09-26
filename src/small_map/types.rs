use core::mem::MaybeUninit;

use hashbrown::HashMap;

/// A compact hash map that stores up to `N` key-value pairs inline and spills
/// to a heap-allocated [`HashMap`] when the inline capacity is exceeded.
///
/// For small maps (≤ `N` entries), all data lives on the stack with no heap
/// allocation.  Lookups use h2-accelerated linear scan.  When the map grows
/// beyond `N` entries it transparently spills to a [`hashbrown::HashMap`].
///
/// The default inline capacity is 8 entries.
///
/// # Examples
/// ```
/// use turbocow::SmallMap;
///
/// let mut map = SmallMap::<&str, i32>::new();
/// map.insert("hello", 1);
/// map.insert("world", 2);
/// assert_eq!(map.get("hello"), Some(&1));
/// assert_eq!(map.len(), 2);
/// ```
///
/// # Inline capacity `N`
///
/// `N` may be any non-zero value. The h2 sidecar is scanned in 16-byte chunks
/// (see `match_h2`), so there is no 32-entry limit;
/// a large `N` simply costs `N × size_of::<(K, V)>` of inline (stack) storage,
/// the same trade-off as `SmallVec`. Past a few dozen entries a heap
/// [`HashMap`] is usually faster — the inline scan is linear.
/// ```
/// use turbocow::SmallMap;
/// let mut big = SmallMap::<u32, u32, 64>::new();
/// for i in 0..50 {
///     big.insert(i, i * 2);
/// }
/// assert_eq!(big.get(&40), Some(&80));
/// assert_eq!(big.len(), 50);
/// ```
pub struct SmallMap<K, V, const N: usize = 8, S = hashbrown::DefaultHashBuilder>(
    pub(super) Repr<K, V, N, S>,
);

pub(super) enum Repr<K, V, const N: usize, S> {
    /// Flat inline storage with h2 sidecar (up to `N` entries).
    Inline(InlineMap<K, V, N, S>),
    /// Heap-allocated hashbrown HashMap.
    Heap(HashMap<K, V, S>),
}

/// Flat inline storage for key-value pairs with h2 sidecar.
///
/// Entries and h2 bytes are stored in fixed-size arrays of capacity `N`.
/// All `N` h2 bytes are initialised to [`H2_EMPTY`](super::group::H2_EMPTY)
/// at construction time, ensuring SIMD reads are always safe.  When the map
/// reaches `N` entries it spills to a hashbrown `HashMap`.
pub struct InlineMap<K, V, const N: usize, S> {
    pub(super) entries: [MaybeUninit<(K, V)>; N],
    pub(super) h2_bytes: [u8; N],
    pub(super) len: usize,
    pub(super) hasher: S,
}

// ── Compile-time invariant ───────────────────────────────────────────────
// N must be non-zero; enforced via a `const { assert!(N > 0) }` in
// `InlineMap::new`. There is no upper bound on N — `match_h2` (group.rs) scans
// the h2 sidecar in 16-byte chunks, so any N is supported.
