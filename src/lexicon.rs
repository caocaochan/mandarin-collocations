use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;
use opencc_fmmseg::{OpenCC, OpenccConfig};

use crate::{Result, open};

#[derive(Default)]
struct Forms {
    traditional: BTreeSet<String>,
    readings: BTreeSet<String>,
}

pub struct Lexicon {
    entries: HashMap<String, Forms>,
    simplified: HashMap<String, BTreeSet<String>>,
    proper_names: HashSet<String>,
    canonical_cache: HashMap<String, String>,
    opencc: OpenCC,
}

impl Lexicon {
    pub fn load(path: &Path) -> Result<Self> {
        let mut lexicon = Self {
            entries: HashMap::new(),
            simplified: HashMap::new(),
            proper_names: HashSet::new(),
            canonical_cache: HashMap::new(),
            opencc: OpenCC::new(),
        };
        let input = BufReader::new(GzDecoder::new(open(path)?));
        for line in input.lines() {
            let line = line?;
            let Some((words, tail)) = line.split_once(" [") else {
                continue;
            };
            let Some((traditional, simplified)) = words.split_once(' ') else {
                continue;
            };
            let Some((pinyin, definitions)) = tail.split_once(']') else {
                continue;
            };
            if !han_word(traditional) || !han_word(simplified) {
                continue;
            }
            let forms = lexicon.entries.entry(simplified.into()).or_default();
            forms.traditional.insert(traditional.into());
            if let Some(reading) = reading(pinyin) {
                forms.readings.insert(reading);
            }
            lexicon
                .simplified
                .entry(traditional.into())
                .or_default()
                .insert(simplified.into());
            if definitions.contains("(proper name)") || definitions.contains("(place name)") {
                lexicon.proper_names.insert(simplified.into());
            }
        }
        if lexicon.entries.len() < 50_000 {
            return Err("CC-CEDICT is incomplete".into());
        }
        Ok(lexicon)
    }

    pub fn canonical(&mut self, word: &str) -> String {
        if let Some(cached) = self.canonical_cache.get(word) {
            return cached.clone();
        }
        let simplified = match self.simplified.get(word) {
            Some(forms) if forms.len() == 1 => forms.first().unwrap().to_owned(),
            _ if self.entries.contains_key(word) => word.to_owned(),
            _ => self
                .opencc
                .convert_with_config(word, OpenccConfig::T2s, false),
        };
        self.canonical_cache
            .insert(word.to_owned(), simplified.clone());
        simplified
    }

    pub fn known(&self, word: &str) -> bool {
        self.entries.contains_key(word)
    }

    pub fn is_name(&self, word: &str) -> bool {
        self.proper_names.contains(word)
    }

    pub fn traditional(&self, word: &str) -> String {
        match self.entries.get(word) {
            Some(forms) if forms.traditional.len() == 1 => {
                forms.traditional.first().unwrap().clone()
            }
            _ => self
                .opencc
                .convert_with_config(word, OpenccConfig::S2t, false),
        }
    }

    pub fn lookup_families<'a>(
        &'a self,
        headwords: impl IntoIterator<Item = &'a String>,
    ) -> BTreeMap<String, BTreeSet<String>> {
        let headwords: HashSet<&str> = headwords.into_iter().map(String::as_str).collect();
        let mut graph = HashMap::<String, Vec<String>>::new();
        let mut connect = |one: &str, other: &str| {
            if one != other {
                graph.entry(one.into()).or_default().push(other.into());
                graph.entry(other.into()).or_default().push(one.into());
            }
        };
        for (word, forms) in &self.entries {
            for alias in &forms.traditional {
                connect(word, alias);
            }
        }
        for (surface, canonical) in &self.canonical_cache {
            connect(surface, canonical);
        }
        for &word in &headwords {
            if !self.entries.contains_key(word) {
                connect(word, &self.traditional(word));
            }
        }
        // Both simplified spellings are used for the same word as 計畫.
        connect("计划", "計畫");
        connect("计划", "计画");

        let mut visited = HashSet::new();
        let mut families = BTreeMap::new();
        for word in headwords.iter().copied() {
            if !visited.insert(word) {
                continue;
            }
            let mut spelling = vec![word];
            let mut family = BTreeSet::new();
            while let Some(alias) = spelling.pop() {
                family.insert(alias.to_owned());
                if let Some(neighbors) = graph.get(alias) {
                    spelling.extend(
                        neighbors
                            .iter()
                            .map(String::as_str)
                            .filter(|form| visited.insert(form)),
                    );
                }
            }
            for member in family
                .iter()
                .filter(|member| headwords.contains(member.as_str()))
            {
                families.insert(member.clone(), family.clone());
            }
        }
        families
    }

    pub fn reading(&self, word: &str) -> &str {
        self.entries
            .get(word)
            .filter(|forms| forms.readings.len() == 1)
            .and_then(|forms| forms.readings.first().map(String::as_str))
            .unwrap_or("")
    }
}

pub fn han_word(word: &str) -> bool {
    !word.is_empty() && word.chars().count() <= 10 && word.chars().all(|ch| {
        matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{20000}'..='\u{323af}' | '〇')
    })
}

fn reading(input: &str) -> Option<String> {
    let mut output = String::new();
    for syllable in input.split_whitespace() {
        let tone = syllable.chars().last()?.to_digit(10)? as usize;
        let syllable = syllable[..syllable.len() - 1]
            .to_lowercase()
            .replace("u:", "ü")
            .replace('v', "ü");
        if !(1..=5).contains(&tone)
            || syllable.is_empty()
            || !syllable
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch == 'ü')
        {
            return None;
        }
        if tone == 5 {
            output.push_str(&syllable);
            continue;
        }
        let position = syllable
            .find('a')
            .or_else(|| syllable.find('e'))
            .or_else(|| syllable.find("ou"))
            .or_else(|| syllable.rfind(['i', 'o', 'u', 'ü']));
        let position = position?;
        let vowel = syllable[position..].chars().next()?;
        let accents = match vowel {
            'a' => ['ā', 'á', 'ǎ', 'à'],
            'e' => ['ē', 'é', 'ě', 'è'],
            'i' => ['ī', 'í', 'ǐ', 'ì'],
            'o' => ['ō', 'ó', 'ǒ', 'ò'],
            'u' => ['ū', 'ú', 'ǔ', 'ù'],
            'ü' => ['ǖ', 'ǘ', 'ǚ', 'ǜ'],
            _ => return None,
        };
        output.push_str(&syllable[..position]);
        output.push(accents[tone - 1]);
        output.push_str(&syllable[position + vowel.len_utf8()..]);
    }
    (!output.is_empty()).then_some(output)
}
