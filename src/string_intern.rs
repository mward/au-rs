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
        StringInternConfig {
            tiny_str: 4,
            intern_thresh: 10,
            intern_cache_size: 1000,
            clear_threshold: 1400,
        }
    }
}

struct UsageTracker {
    intern_thresh: usize,
    intern_cache_size: usize,
    /// Ordered list of recently seen strings (front = oldest)
    in_order: Vec<String>,
    /// Maps string -> (count, index_in_in_order)
    dict: HashMap<String, (usize, usize)>,
}

impl UsageTracker {
    fn new(intern_thresh: usize, intern_cache_size: usize) -> Self {
        UsageTracker {
            intern_thresh,
            intern_cache_size,
            in_order: Vec::new(),
            dict: HashMap::new(),
        }
    }

    fn should_intern(&mut self, sv: &str) -> bool {
        if let Some(entry) = self.dict.get_mut(sv) {
            if entry.0 >= self.intern_thresh {
                // Remove from tracking
                let idx = entry.1;
                self.dict.remove(sv);
                // Mark slot as empty by clearing the string
                if idx < self.in_order.len() {
                    self.in_order[idx] = String::new();
                }
                return true;
            } else {
                entry.0 += 1;
                return false;
            }
        }

        // Evict oldest if at capacity
        if self.dict.len() >= self.intern_cache_size {
            // Find the oldest non-empty entry
            for i in 0..self.in_order.len() {
                if !self.in_order[i].is_empty() {
                    let key = std::mem::take(&mut self.in_order[i]);
                    self.dict.remove(&key);
                    break;
                }
            }
        }

        let idx = self.in_order.len();
        let s = sv.to_string();
        self.dict.insert(s.clone(), (1, idx));
        self.in_order.push(s);
        false
    }

    fn clear(&mut self) {
        self.dict.clear();
        self.in_order.clear();
    }

    fn size(&self) -> usize {
        self.dict.len()
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
    pub fn new() -> Self {
        Self::with_config(StringInternConfig::default())
    }

    pub fn with_config(config: StringInternConfig) -> Self {
        let reserve_size = (config.clear_threshold as f64 * 1.2) as usize;
        StringIntern {
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
            .dictionary.values().map(|entry| {
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

    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("HashSize".to_string(), self.dictionary.len());
        stats.insert("DictSize".to_string(), self.dict_in_order.len());
        stats.insert("CacheSize".to_string(), self.intern_cache.size());
        stats
    }
}

impl Default for StringIntern {
    fn default() -> Self {
        Self::new()
    }
}
