//! The horizontal scroll total must be measured *lazily*.
//!
//! `ui::scrollbar::content_width` walks a `RopeSlice`'s characters and stops at
//! `SCAN_BUDGET`. The obvious-looking alternative -- collect the line into a
//! `String` and measure that -- gives the identical answer for every input the
//! rest of the suite feeds it, because on pure ASCII the width and the
//! character count agree. It is only the *cost* that differs, and it differs by
//! everything: the allocation and the copy happen before the loop that stops,
//! so the scan becomes O(line length) whatever the cap says. That was the
//! original defect (26.8 ms per call on a five-million-column line, once per
//! visible line per frame), and no assertion on a returned width can see it
//! come back.
//!
//! So this measures the cost directly, through a counting global allocator.
//! `ropey::RopeSlice::chars()` allocates nothing, so what is counted is the
//! scan's own doing and a collecting implementation shows up as the whole line.
//!
//! **This file holds one test on purpose.** The counter is process-wide, and
//! libtest runs the tests of a binary on parallel threads: a second test here
//! would have its allocations charged to this one.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use termcode_term::ui::scrollbar::{SCAN_BUDGET, content_width};
use termcode_view::document::{Document, DocumentId};

static BYTES: AtomicUsize = AtomicUsize::new(0);
static COUNTING: AtomicBool = AtomicBool::new(false);

/// The system allocator, plus a byte counter that is only armed around the call
/// under test. Armed for the whole run it would also charge the harness's
/// startup and the fixtures' own (large, and deliberate) allocations.
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    // `realloc` and `alloc_zeroed` are deliberately not overridden: the trait's
    // default implementations route through `alloc` above, so a `String` that
    // grows by doubling is counted at every step rather than slipping past.
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Bytes allocated on this thread while `f` runs.
fn bytes_allocated<T>(f: impl FnOnce() -> T) -> (T, usize) {
    BYTES.store(0, Ordering::SeqCst);
    COUNTING.store(true, Ordering::SeqCst);
    let out = f();
    COUNTING.store(false, Ordering::SeqCst);
    (out, BYTES.load(Ordering::SeqCst))
}

fn doc_with_one_line_of(columns: usize) -> Document {
    let mut doc = Document::new(DocumentId(0));
    doc.buffer.text_mut().insert(0, &"x".repeat(columns));
    doc
}

#[test]
fn measuring_the_scroll_total_does_not_allocate_the_line_it_measures() {
    const CODE_WIDTH: usize = 54;
    let small = SCAN_BUDGET * 10;
    let large = small * 10;

    let short = doc_with_one_line_of(small);
    let long = doc_with_one_line_of(large);

    let (short_total, short_bytes) = bytes_allocated(|| content_width(&short, 0, 1, CODE_WIDTH));
    let (long_total, long_bytes) = bytes_allocated(|| content_width(&long, 0, 1, CODE_WIDTH));

    // Both lines are past the budget, so both answer with it -- which is
    // exactly why the returned value cannot tell the two implementations
    // apart, and why the bytes have to.
    assert_eq!(short_total, SCAN_BUDGET);
    assert_eq!(long_total, SCAN_BUDGET);

    // A line ten times longer must not cost ten times more. Stated as a bound
    // rather than as equality: the harness thread is parked on a channel while
    // this runs, but "allocates nothing at all in the background" is not a
    // promise the runtime makes, and a flaky guard is a deleted guard.
    assert!(
        long_bytes <= short_bytes + 4096,
        "the scan's cost tracks the line's length: {small} columns cost \
         {short_bytes} bytes, {large} columns cost {long_bytes}"
    );

    // And in absolute terms: neither call may allocate anything on the order of
    // a line. Collecting `long` into a `String` alone is `large` bytes.
    assert!(
        long_bytes < 64 * 1024,
        "measuring a {large}-column line allocated {long_bytes} bytes -- the \
         line is being collected rather than walked"
    );
}
