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
                "i = {}",
                i
            );
            assert_eq!(0, si.dict().len(), "i = {}", i);
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
