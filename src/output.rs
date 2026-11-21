use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::Result;
use crate::lexicon::Lexicon;
use crate::pairs::{Collocate, Entries, Group};

const TITLE: &str = "華語搭配 · Mandarin Collocations";
const BANK_SIZE: usize = 1_000;
const REPOSITORY: &str = "https://github.com/caocaochan/mandarin-collocations";
const ATTRIBUTION: &str = "Hu & Xiao, Chinese Collocation Knowledgebase (education and Chinese Wikipedia). Taiwanese Mandarin web n-grams by Den Patin (CC0 1.0). MagicData-RAMC by Magic Data Technology (CC BY-NC-ND 4.0; locally derived counts). CC-CEDICT (CC BY-SA 4.0) for written forms and readings. OpenCC for fallback script conversion.";

pub struct Report {
    pub forms: usize,
    pub headwords: usize,
}

pub fn write(
    path: &Path,
    styles: &str,
    lexicon: &Lexicon,
    entries: &Entries,
    observed_pairs: usize,
) -> Result<Report> {
    let parent = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("partial");
    let mut archive = ZipWriter::new(BufWriter::new(File::create(&temporary)?));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let revision = std::env::var("MANDARIN_COLLOCATIONS_REVISION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned());
    let version: Vec<_> = revision
        .split('.')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<_, _>>()?;
    if version.len() != 3 || format!("{}.{}.{}", version[0], version[1], version[2]) != revision {
        return Err("Revision must use three numeric components, for example 1.1.0".into());
    }
    let metadata = serde_json::to_vec_pretty(&json!({
        "title": TITLE, "format": 3, "revision": revision, "sequenced": true,
        "sourceLanguage": "zh", "targetLanguage": "zh",
        "description": "現代華語常用搭配。正體、簡體與收錄的異體可查同一組詞條。",
        "url": REPOSITORY,
        "attribution": ATTRIBUTION,
        "isUpdatable": true,
        "indexUrl": format!("{REPOSITORY}/releases/latest/download/index.json"),
        "downloadUrl": format!("{REPOSITORY}/releases/latest/download/mandarin-collocations.zip"),
    }))?;
    archive.start_file("index.json", options)?;
    archive.write_all(&metadata)?;
    archive.start_file("styles.css", options)?;
    archive.write_all(styles.as_bytes())?;

    let aliases = lexicon.lookup_families(entries.keys());
    let forms: HashSet<&str> = aliases
        .values()
        .flat_map(|forms| forms.iter().map(String::as_str))
        .collect();
    let mut bank = Vec::with_capacity(BANK_SIZE);
    let mut bank_number = 0;
    let mut forms_written = 0;
    for (sequence, (headword, groups)) in entries.iter().enumerate() {
        let traditional = lexicon.traditional(headword);
        let glossary = render(headword, &traditional, groups, &forms, lexicon);
        for form in &aliases[headword] {
            bank.push(json!([
                form,
                lexicon.reading(headword),
                "",
                "",
                0,
                [glossary],
                sequence + 1,
                ""
            ]));
            forms_written += 1;
            if bank.len() == BANK_SIZE {
                flush(&mut archive, &mut bank, &mut bank_number, options)?;
            }
        }
    }
    if !bank.is_empty() {
        flush(&mut archive, &mut bank, &mut bank_number, options)?;
    }
    archive.finish()?.into_inner()?.sync_all()?;
    fs::rename(temporary, path)?;
    fs::write(parent.join("index.json"), &metadata)?;

    let mut pair_directions = 0;
    let mut directions_by_source = [0; 4];
    for collocate in entries.values().flat_map(BTreeMap::values).flatten() {
        pair_directions += 1;
        for (supported, evidence) in directions_by_source.iter_mut().zip(collocate.evidence) {
            *supported += usize::from(evidence.count > 0);
        }
    }
    let summary = json!({"headwords": entries.len(), "lookup_forms": forms_written, "term_banks": bank_number,
        "observed_pairs_before_filtering": observed_pairs, "displayed_pair_directions": pair_directions,
        "displayed_directions_with_attestation": {"education": directions_by_source[0], "wikipedia": directions_by_source[1],
            "taiwanese_web": directions_by_source[2], "mainland_speech": directions_by_source[3]}});
    serde_json::to_writer_pretty(File::create(parent.join("build-summary.json"))?, &summary)?;
    Ok(Report {
        forms: forms_written,
        headwords: entries.len(),
    })
}

fn flush(
    archive: &mut ZipWriter<BufWriter<File>>,
    bank: &mut Vec<Value>,
    number: &mut usize,
    options: SimpleFileOptions,
) -> Result<()> {
    *number += 1;
    archive.start_file(format!("term_bank_{number}.json"), options)?;
    serde_json::to_writer(archive, &bank)?;
    bank.clear();
    Ok(())
}

fn render(
    headword: &str,
    traditional: &str,
    groups: &BTreeMap<Group, Vec<Collocate>>,
    forms: &HashSet<&str>,
    lexicon: &Lexicon,
) -> Value {
    let mut badges = vec![node("span", json!(traditional), "traditional-term")];
    if headword != traditional {
        badges.push(node("span", json!(headword), "simplified-term"));
    }
    let mut body = vec![node(
        "span",
        json!([node("span", json!(badges), "terms-parent")]),
        "first-row-parent",
    )];
    let mut ordered: Vec<_> = groups.iter().collect();
    ordered.sort_by_key(|(group, _)| (group.relation, group.reverse, sense_order(group.sense)));
    body.extend(render_groups(traditional, &ordered, forms, lexicon));
    json!({"type": "structured-content", "content": {
        "tag": "div", "data": {"content": "hydcd-entry"}, "content": body,
        "lang": "zh-Hant",
    }})
}

fn render_groups(
    displayed_word: &str,
    ordered: &[(&Group, &Vec<Collocate>)],
    forms: &HashSet<&str>,
    lexicon: &Lexicon,
) -> Vec<Value> {
    let mut blocks = Vec::with_capacity(ordered.len());
    for &(group, collocates) in ordered {
        let mut phrases = Vec::with_capacity(collocates.len() * 2);
        for collocate in collocates {
            if !phrases.is_empty() {
                phrases.push(json!("、"));
            }
            let partner = lexicon.traditional(&collocate.partner);
            let target = if forms.contains(partner.as_str()) {
                partner.as_str()
            } else {
                collocate.partner.as_str()
            };
            let link = json!({"tag": "a", "href": format!("?query={}", url_encode(target)), "content": partner});
            let pair = if group.reverse {
                json!([link, displayed_word])
            } else {
                json!([displayed_word, link])
            };
            phrases
                .push(json!({"tag": "span", "style": {"whiteSpace": "nowrap"}, "content": pair}));
        }
        let label = group.relation.labels(group.reverse);
        let label = if group.sense.is_empty() {
            label.to_owned()
        } else {
            format!("{label} · {}", group.sense)
        };
        blocks.push(node(
            "div",
            json!([
                node("div", json!(label), "header"),
                node("div", json!(phrases), "subsense"),
            ]),
            "sense",
        ));
    }
    blocks
}

fn node(tag: &str, content: Value, kind: &str) -> Value {
    json!({"tag": tag, "content": content, "data": {"content": kind}})
}

fn sense_order(sense: &str) -> u8 {
    match sense {
        "課業" => 0,
        "工作、操作" => 1,
        _ => 2,
    }
}

fn url_encode(word: &str) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut escaped = String::with_capacity(word.len() * 3);
    for byte in word.bytes() {
        escaped.push('%');
        escaped.push(HEX[(byte >> 4) as usize] as char);
        escaped.push(HEX[(byte & 15) as usize] as char);
    }
    escaped
}
