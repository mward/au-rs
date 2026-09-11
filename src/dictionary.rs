use crate::error::ParseError;

pub struct Dict {
    dictionary: Vec<String>,
    start_pos: usize,
    last_dict_pos: usize,
}

impl Dict {
    pub fn new(start_pos: usize) -> Self {
        Self {
            dictionary: Vec::with_capacity(1 << 16),
            start_pos,
            last_dict_pos: start_pos,
        }
    }

    pub fn reset(&mut self, sor: usize) {
        self.dictionary.clear();
        self.start_pos = sor;
        self.last_dict_pos = sor;
    }

    pub fn add(&mut self, sor: usize, value: String) {
        self.dictionary.push(value);
        self.last_dict_pos = sor;
    }

    pub const fn includes(&self, sor: usize) -> bool {
        self.start_pos <= sor && sor <= self.last_dict_pos
    }

    pub fn at(&self, idx: usize) -> Result<&str, ParseError> {
        if idx >= self.dictionary.len() {
            return Err(ParseError::new(format!(
                "Dictionary reference index {} out of range. Dictionary started at position {}, \
                 last add occurred at position {}, and currently has {} entries.",
                idx,
                self.start_pos,
                self.last_dict_pos,
                self.dictionary.len()
            )));
        }
        Ok(&self.dictionary[idx])
    }

    pub const fn size(&self) -> usize {
        self.dictionary.len()
    }
}

pub struct Dictionary {
    dictionaries: Vec<Dict>,
    max_dicts: usize,
}

impl Dictionary {
    pub fn new() -> Self {
        Self::with_max_dicts(1)
    }

    pub fn with_max_dicts(max_dicts: usize) -> Self {
        Self {
            dictionaries: Vec::with_capacity(max_dicts),
            max_dicts,
        }
    }

    pub fn clear(&mut self, sor: usize) -> &mut Dict {
        if let Some(idx) = self.dictionaries.iter().position(|d| d.start_pos == sor) {
            return &mut self.dictionaries[idx];
        }

        if self.dictionaries.len() == self.max_dicts {
            let mut recycled = self.dictionaries.remove(0);
            recycled.reset(sor);
            self.dictionaries.push(recycled);
        } else {
            self.dictionaries.push(Dict::new(sor));
        }
        self.dictionaries.last_mut().unwrap()
    }

    pub fn find_dictionary_ref(
        &self,
        sor: usize,
        rel_dict_pos: usize,
    ) -> Result<&Dict, ParseError> {
        let pos = sor - rel_dict_pos;
        if let Some(dict) = self.dictionaries.iter().rev().find(|d| d.includes(pos)) {
            return Ok(dict);
        }
        Err(ParseError::new(format!(
            "wrong backref: no dictionary includes absolute position = {pos}: \
             start-of-record = {sor} relDictPos = {rel_dict_pos}"
        )))
    }

    pub fn find_dictionary(
        &mut self,
        sor: usize,
        rel_dict_pos: usize,
    ) -> Result<&mut Dict, ParseError> {
        let pos = sor - rel_dict_pos;
        if let Some(idx) = self.dictionaries.iter().rposition(|d| d.includes(pos)) {
            return Ok(&mut self.dictionaries[idx]);
        }
        Err(ParseError::new(format!(
            "wrong backref: no dictionary includes absolute position = {pos}: \
             start-of-record = {sor} relDictPos = {rel_dict_pos}"
        )))
    }

    pub fn latest(&mut self) -> Option<&mut Dict> {
        self.dictionaries.last_mut()
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new()
    }
}
