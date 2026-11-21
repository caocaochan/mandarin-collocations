use std::io::{BufRead, BufReader};
use std::path::Path;

use calamine::{DataType, Reader, Xlsx, open_workbook};
use zip::ZipArchive;

use crate::lexicon::{Lexicon, han_word};
use crate::pairs::{Pairs, Relation, Source};
use crate::{Result, open};

pub fn load(inputs: &Path, lexicon: &mut Lexicon, pairs: &mut Pairs) -> Result<()> {
    let wiki_rows = load_wiki(
        &inputs.join("wiki_collocation_data.txt.zip"),
        lexicon,
        pairs,
    )?;
    let education_rows = load_education(&inputs.join("edu_collocation_data.xlsx"), lexicon, pairs)?;
    let speech_rows = load_speech(&inputs.join("speech.tsv"), lexicon, pairs)?;
    load_reviews(pairs)?;
    let taiwan_rows = load_taiwan(&inputs.join("twngrams.tsv"), lexicon, pairs)?;
    eprintln!(
        "Matched source rows: {wiki_rows} Wikipedia, {education_rows} education, {speech_rows} speech, {taiwan_rows} Taiwanese web"
    );
    Ok(())
}

fn load_wiki(path: &Path, lexicon: &mut Lexicon, pairs: &mut Pairs) -> Result<usize> {
    let mut archive = ZipArchive::new(open(path)?)?;
    let input = BufReader::new(archive.by_name("wiki_collocation_data.txt")?);
    let mut accepted = 0;
    for (index, line) in input.lines().enumerate() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7 {
            continue;
        }
        accepted += usize::from(load_hu_row(
            pairs,
            lexicon,
            Source::Wiki,
            fields[1],
            fields[2],
            fields[4]
                .parse::<i32>()
                .map_err(|error| format!("Wikipedia frequency at row {}: {error}", index + 1))?
                .unsigned_abs(),
            fields[6]
                .parse()
                .map_err(|error| format!("Wikipedia association at row {}: {error}", index + 1))?,
        ));
    }
    Ok(accepted)
}

fn load_education(path: &Path, lexicon: &mut Lexicon, pairs: &mut Pairs) -> Result<usize> {
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let sheet = workbook
        .worksheet_range_at(0)
        .ok_or("Education workbook has no worksheet")??;
    let mut rows = sheet.rows();
    let header = rows.next().ok_or("Education worksheet is empty")?;
    if header.get(1).and_then(DataType::get_string) != Some("Collocation")
        || header.get(4).and_then(DataType::get_string) != Some("Frequency")
    {
        return Err("Unexpected education worksheet columns".into());
    }
    let mut accepted = 0;
    for row in rows {
        let (Some(phrase), Some(code), Some(count), Some(association)) = (
            row.get(1).and_then(DataType::get_string),
            row.get(2).and_then(DataType::get_string),
            row.get(4).and_then(DataType::as_i64),
            row.get(6).and_then(DataType::as_f64),
        ) else {
            continue;
        };
        accepted += usize::from(load_hu_row(
            pairs,
            lexicon,
            Source::Education,
            phrase,
            code,
            u32::try_from(count)?,
            association as f32,
        ));
    }
    Ok(accepted)
}

fn load_hu_row(
    pairs: &mut Pairs,
    lexicon: &mut Lexicon,
    source: Source,
    phrase: &str,
    code: &str,
    count: u32,
    association: f32,
) -> bool {
    let Some(relation) = Relation::from_code(code) else {
        return false;
    };
    let words: Vec<_> = phrase.split_whitespace().collect();
    let (left, right) = match (code, words.as_slice()) {
        ("V_O" | "S_V" | "A_N" | "D_V" | "D_A" | "Q_N" | "V_C", [left, right]) => (*left, *right),
        ("A_DE_N", [left, "的", right]) => (*left, *right),
        // Keep attestation from transparent surface variants without double-counting their base row.
        ("V_HV_O" | "V_2HV_O", [left, middle @ .., right])
            if !middle.is_empty()
                && middle.iter().all(|word| {
                    matches!(*word, "了" | "过" | "過" | "着" | "著" | "完" | "到" | "好")
                }) =>
        {
            (*left, *right)
        }
        ("S_V_HV" | "S_V_2HV", [left, right, middle @ ..])
            if !middle.is_empty()
                && middle
                    .iter()
                    .all(|word| matches!(*word, "了" | "过" | "過" | "着" | "著")) =>
        {
            (*left, *right)
        }
        _ => return false,
    };
    if !han_word(left) || !han_word(right) || count == 0 || !association.is_finite() {
        return false;
    }
    pairs.observe(
        lexicon.canonical(left),
        lexicon.canonical(right),
        relation,
        source,
        count,
        association,
    );
    true
}

fn load_speech(path: &Path, lexicon: &mut Lexicon, pairs: &mut Pairs) -> Result<usize> {
    let input = BufReader::new(open(path)?);
    let mut count = 0;
    for line in input.lines().skip(1) {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        let [code, left, right, conversations] = fields.as_slice() else {
            return Err("Invalid speech.tsv row".into());
        };
        let relation = Relation::from_code(code).ok_or("Invalid speech relation")?;
        if han_word(left) && han_word(right) {
            pairs.observe(
                lexicon.canonical(left),
                lexicon.canonical(right),
                relation,
                Source::Speech,
                conversations.parse()?,
                0.0,
            );
            count += 1;
        }
    }
    if count == 0 {
        return Err("No spoken collocations in speech.tsv".into());
    }
    Ok(count)
}

fn load_taiwan(path: &Path, lexicon: &mut Lexicon, pairs: &mut Pairs) -> Result<usize> {
    let input = BufReader::new(open(path)?);
    let mut matched = 0;
    for line in input.lines() {
        let line = line?;
        let Some((count, phrase)) = line.split_once('\t') else {
            continue;
        };
        let Some((left, right)) = phrase.split_once(' ') else {
            continue;
        };
        if han_word(left)
            && han_word(right)
            && pairs.add_taiwan(
                lexicon.canonical(left),
                lexicon.canonical(right),
                count.parse()?,
            )
        {
            matched += 1;
        }
    }
    Ok(matched)
}

fn load_reviews(pairs: &mut Pairs) -> Result<()> {
    for line in include_str!("../reviews.tsv").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        let (action, code, left, right, sense) = match fields.as_slice() {
            [action, code, left, right] => (*action, *code, *left, *right, ""),
            [action, code, left, right, sense] => (*action, *code, *left, *right, *sense),
            _ => return Err(format!("Invalid review row: {line}").into()),
        };
        let relation = Relation::from_code(code).ok_or("Invalid review relation")?;
        if !pairs.review(left, right, relation, action, sense) {
            return Err(format!("Invalid review row: {line}").into());
        }
    }
    Ok(())
}
