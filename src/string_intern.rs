use schnellru::{ByLength, LruMap};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternMode {
    ByFrequency,
    ForceIntern,
    ForceExplicit,
}

pub struct StringInternConfig {
    pub tiny_str: usize,
    pub intern_thresh: usize,
    pub intern_cache_size: usize,
    pub clear_threshold: usize,
}

impl Default for StringInternConfig {
    fn default() -> Self {
        Self {
            tiny_str: 4,
            intern_thresh: 10,
            intern_cache_size: 1000,
            clear_threshold: 1400,
        }
    }
}

/// Tracks how often a candidate string has been seen, evicting the
/// oldest-inserted entries once the cache is full. Backed by a bounded map so
/// lookups and evictions are O(1) and memory stays capped at the cache size (no
/// accumulating tombstones). Eviction is insertion-order (FIFO): counting a
/// string does not refresh its position, matching the prior windowing policy.
struct UsageTracker {
    intern_thresh: usize,
    /// Maps candidate string -> times seen; capped at the cache size.
    counts: LruMap<String, usize>,
}

impl UsageTracker {
    fn new(intern_thresh: usize, intern_cache_size: usize) -> Self {
        // Fixed-seed hasher: this internal cache is not a HashDoS boundary, so
        // we drop the randomized `runtime-rng` feature (and its getrandom dep).
        const SEED: [u64; 4] = [
            0x243f_6a88_85a3_08d3,
            0x1319_8a2e_0370_7344,
            0xa409_3822_299f_31d0,
            0x082e_fa98_ec4e_6c89,
        ];
        // Saturate rather than wrap: a cache size >= 2^32 would otherwise
        // truncate (and a value that wraps to 0 would disable interning).
        let cap = u32::try_from(intern_cache_size).unwrap_or(u32::MAX);
        Self {
            intern_thresh,
            counts: LruMap::with_seed(ByLength::new(cap), SEED),
        }
    }

    fn should_intern(&mut self, sv: &str) -> bool {
        // `peek_mut` reads/updates the count without touching eviction order, so
        // eviction stays FIFO (oldest-inserted), matching the prior behavior.
        match self.counts.peek_mut(sv) {
            Some(count) if *count >= self.intern_thresh => {
                // Threshold reached: promote to the real dictionary and stop tracking.
                self.counts.remove(sv);
                true
            }
            Some(count) => {
                *count += 1;
                false
            }
            None => {
                // Inserting at capacity evicts the oldest-inserted entry.
                self.counts.insert(sv.to_string(), 1);
                false
            }
        }
    }

    fn clear(&mut self) {
        self.counts.clear();
    }

    fn size(&self) -> usize {
        self.counts.len()
    }
}

struct InternEntry {
    intern_index: usize,
    occurrences: usize,
}

pub struct StringIntern {
    dict_in_order: Vec<String>,
    dictionary: HashMap<String, InternEntry>,
    tiny_string_size: usize,
    intern_cache: UsageTracker,
}

impl StringIntern {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(StringInternConfig::default())
    }

    #[must_use]
    pub fn with_config(config: StringInternConfig) -> Self {
        let reserve_size = (config.clear_threshold as f64 * 1.2) as usize;
        Self {
            dict_in_order: Vec::with_capacity(reserve_size),
            dictionary: HashMap::with_capacity(reserve_size),
            tiny_string_size: config.tiny_str,
            intern_cache: UsageTracker::new(config.intern_thresh, config.intern_cache_size),
        }
    }

    pub fn idx(&mut self, sv: &str, intern: InternMode) -> Option<usize> {
        if sv.len() <= self.tiny_string_size {
            return None;
        }
        if intern == InternMode::ForceExplicit {
            return None;
        }

        if let Some(entry) = self.dictionary.get_mut(sv) {
            entry.occurrences += 1;
            return Some(entry.intern_index);
        }

        if intern == InternMode::ForceIntern || self.intern_cache.should_intern(sv) {
            let next_entry = self.dict_in_order.len();
            let s = sv.to_string();
            self.dictionary.insert(
                s.clone(),
                InternEntry {
                    intern_index: next_entry,
                    occurrences: 1,
                },
            );
            self.dict_in_order.push(s);
            return Some(next_entry);
        }
        None
    }

    #[must_use]
    pub fn dict(&self) -> &[String] {
        &self.dict_in_order
    }

    pub fn clear(&mut self, clear_usage_tracker: bool) {
        self.dictionary.clear();
        self.dict_in_order.clear();
        if clear_usage_tracker {
            self.intern_cache.clear();
        }
    }

    pub fn purge(&mut self, threshold: usize) -> usize {
        let mut purged = 0;
        self.dictionary.retain(|_, entry| {
            if entry.occurrences < threshold {
                purged += 1;
                false
            } else {
                true
            }
        });
        purged
    }

    pub fn reindex(&mut self, threshold: usize) -> usize {
        let purged = self.purge(threshold);
        self.do_reindex();
        purged
    }

    fn do_reindex(&mut self) {
        let mut tmp_dict: Vec<(usize, String)> = self
            .dictionary
            .values()
            .map(|entry| {
                (
                    entry.occurrences,
                    self.dict_in_order[entry.intern_index].clone(),
                )
            })
            .collect();

        // Sort descending by occurrences, then by string for stability
        tmp_dict.sort_by(|a, b| b.cmp(a));

        self.dict_in_order.clear();
        self.dictionary.clear();
        for (idx, (occurrences, s)) in tmp_dict.into_iter().enumerate() {
            self.dictionary.insert(
                s.clone(),
                InternEntry {
                    intern_index: idx,
                    occurrences,
                },
            );
            self.dict_in_order.push(s);
        }
    }

    #[must_use]
    pub fn get_stats(&self) -> InternStats {
        InternStats {
            hash_size: self.dictionary.len(),
            dict_size: self.dict_in_order.len(),
            cache_size: self.intern_cache.size(),
        }
    }
}

/// Snapshot of a `StringIntern`'s internal sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternStats {
    pub hash_size: usize,
    pub dict_size: usize,
    pub cache_size: usize,
}

impl Default for StringIntern {
    fn default() -> Self {
        Self::new()
    }
}
