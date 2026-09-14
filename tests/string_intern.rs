//! AuStringIntern tests (ported from AuUnitTests.cpp).

use au::string_intern::{InternMode, StringIntern, StringInternConfig};

#[test]
fn string_intern_no_intern() {
    let mut si = StringIntern::new();
    assert_eq!(0, si.dict().len());
    assert!(si.idx("shrt", InternMode::ByFrequency).is_none());
    assert!(si.idx("Long string", InternMode::ByFrequency).is_none());
    assert_eq!(0, si.dict().len());
}

#[test]
fn string_intern_force_intern() {
    let mut si = StringIntern::new();
    assert_eq!(0, si.dict().len());

    // Tiny strings are not interned even if forced
    assert!(si.idx("tiny", InternMode::ForceIntern).is_none());
    assert_eq!(0, si.dict().len());

    assert!(si.idx("A normal string", InternMode::ForceIntern).is_some());
    assert_eq!(1, si.dict().len());
}

#[test]
fn string_intern_frequent_strings() {
    const INTERN_THRESH: usize = 10;
    let mut si = StringIntern::with_config(StringInternConfig {
        tiny_str: 4,
        intern_thresh: INTERN_THRESH,
        ..Default::default()
    });
    let str_val = "Normal value";

    assert!(si.idx(str_val, InternMode::ByFrequency).is_none());
    assert_eq!(0, si.dict().len());

    for i in 0..(INTERN_THRESH * 2) {
        if i < INTERN_THRESH - 1 {
            assert!(
                si.idx(str_val, InternMode::ByFrequency).is_none(),
                "i = {i}"
            );
            assert_eq!(0, si.dict().len(), "i = {i}");
        } else {
            assert!(si.idx(str_val, InternMode::ByFrequency).is_some());
            assert_eq!(1, si.dict().len());
        }
    }
}

#[test]
fn string_intern_reindex() {
    let mut si = StringIntern::with_config(StringInternConfig {
        tiny_str: 1,
        intern_thresh: 2,
        intern_cache_size: 10,
        ..Default::default()
    });

    si.idx("twice", InternMode::ForceIntern); // idx 0
    si.idx("once", InternMode::ForceIntern); // idx 1
    si.idx("thrice", InternMode::ForceIntern); // idx 2
    si.idx("twice", InternMode::ForceIntern);
    si.idx("thrice", InternMode::ForceIntern);
    si.idx("thrice", InternMode::ForceIntern);

    assert_eq!(3, si.dict().len());
    assert_eq!("twice", si.dict()[0]);
    assert_eq!("once", si.dict()[1]);
    assert_eq!("thrice", si.dict()[2]);

    assert_eq!(1, si.reindex(2));

    assert_eq!(2, si.dict().len());
    assert_eq!("thrice", si.dict()[0]);
    assert_eq!("twice", si.dict()[1]);

    assert_eq!(Some(0), si.idx("thrice", InternMode::ForceIntern));
    assert_eq!(Some(1), si.idx("twice", InternMode::ForceIntern));

    si.idx("quadrice", InternMode::ForceIntern);
    assert_eq!(Some(2), si.idx("quadrice", InternMode::ForceIntern));
}

/// Adapted from C++ `AuStringIntern.NextEntryValidAfterPurgeTriggeredDoReIndex`.
///
/// The original guards a C++-specific bug: `idx()` there triggers a `doReIndex`
/// when `dictInOrder_` hits capacity, and a stale `nextEntry` (captured before
/// the reindex shrank the vector) produced an out-of-bounds intern index.
///
/// au-rs cannot hit that exact bug — `idx()` never reindexes; `dict_in_order` is
/// a growable `Vec` and `next_entry` is read at push time, so it's always valid.
/// What this test still exercises (and what the C++ assertions ultimately check)
/// is the invariant that after `purge` diverges the map from `dict_in_order`,
/// interning new strings and then `reindex`-ing keeps every intern index a valid
/// index into `dict()` and keeps strings findable.
#[test]
fn next_entry_valid_after_purge_then_reindex() {
    let mut si = StringIntern::with_config(StringInternConfig {
        tiny_str: 1,
        intern_thresh: 1,
        intern_cache_size: 100,
        clear_threshold: 20,
    });

    // Fill 20 entries.
    for i in 0..20 {
        si.idx(&format!("entry_{i}"), InternMode::ForceIntern);
    }
    assert_eq!(20, si.dict().len());

    // Bump occurrence counts on the first 15 so they survive the purge.
    for _ in 0..5 {
        for i in 0..15 {
            si.idx(&format!("entry_{i}"), InternMode::ForceIntern);
        }
    }

    // Purge entries seen < 2 times: removes entries 15-19 from the map, while
    // dict_in_order keeps all 20 -> the two structures diverge.
    si.purge(2);

    // Intern 4 more new strings against the diverged state.
    for i in 20..24 {
        si.idx(&format!("entry_{i}"), InternMode::ForceIntern);
    }

    // Intern one more; its index must be valid and resolve back to the string.
    let result = si.idx("trigger_string_xx", InternMode::ForceIntern);
    let idx = result.expect("force-intern of a normal string returns an index");
    assert!(
        idx < si.dict().len(),
        "intern index {idx} out of bounds for dict size {}",
        si.dict().len()
    );
    assert_eq!("trigger_string_xx", si.dict()[idx]);

    // A subsequent reindex reads dict_in_order[intern_index] for every entry;
    // a stale index would be out of bounds here. It must not panic.
    si.reindex(1);

    // The string is still findable after reindex, at a valid index.
    let result2 = si.idx("trigger_string_xx", InternMode::ForceIntern);
    let idx2 = result2.expect("string still interned after reindex");
    assert!(idx2 < si.dict().len());
    assert_eq!("trigger_string_xx", si.dict()[idx2]);
}
