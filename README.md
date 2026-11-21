# 華語搭配 · Mandarin Collocations

Bidirectional Yomitan dictionary for modern Mandarin. Import the [latest dictionary ZIP](https://github.com/caocaochan/mandarin-collocations/releases/latest/download/mandarin-collocations.zip) under **Settings → Dictionaries → Import**. Searching a verb shows objects it takes, and searching a noun shows verbs and modifiers that go with it.

Traditional and Simplified lookup forms return the same entry set, including where several Traditional forms share a Simplified spelling. Each entry displays its collocations in Traditional Chinese. A shared spelling can return entries associated with more than one sense or reading.

The release ZIP includes update metadata. Yomitan checks for a higher numeric revision at the [latest release index](https://github.com/caocaochan/mandarin-collocations/releases/latest/download/index.json). If you imported the original local 1.0.0 build, replace it once with the release ZIP to enable future updates.

The popup uses the shared `styles.css` from [HYDCD](https://github.com/caocaochan/hydcd-yomitan), [Chengyu](https://github.com/caocaochan/chengyu-yomitan), and [NAER](https://github.com/caocaochan/naer-yomitan), including the original script badges and group styling.

## Sources and ranking

- [Hu and Xiao’s Chinese Collocation Knowledgebase](https://github.com/iris2hu/Chinese-collocation-complexity/tree/main/collocation_data) supplies typed pairs and statistics from two collections. The upstream files are publicly available and carry no explicit dataset license in their documentation. The release maintainer confirmed redistribution permission for this build.
- [Den Patin’s Taiwanese Mandarin web n-grams](https://huggingface.co/datasets/taiwan-corpora/twngrams) supplies independent website counts for pairs with grammatical evidence. The published counts are CC0 and have a 40-site floor.
- [MagicData-RAMC](https://www.openslr.org/123/) supplies spoken-Mandarin evidence. The build reads the [text-only Hugging Face Parquet conversion](https://huggingface.co/datasets/EaseZh/magicdata_ramc), parses the utterances with spaCy’s [`zh_core_web_md`](https://spacy.io/models/zh), and counts distinct conversations per pair. The ZIP contains ranked phrases. The source transcripts stay local under CC BY-NC-ND 4.0.
- [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cc-cedict), under CC BY-SA 4.0, supplies script mappings and unambiguous pinyin. [OpenCC dictionaries](https://github.com/BYVoid/OpenCC) supply fallback script forms.

Collocates are ranked within each headword and relation. The Rust builder combines ranks from the four sources, weights repeated evidence, and gives a bounded contribution to association scores. It shows at most eight collocates per group. Pairs are indexed in both directions before truncation, so their prominence may differ under the two headwords. The automatic data groups by grammatical relation. `reviews.tsv` adds a small set of lexical corrections and explicitly separates homework from operational uses of 作業.

Reviewed additions for 釐清 are attested by the [Taiwan MOE dictionary](https://dict.revised.moe.edu.tw/dictView.jsp?ID=61796&word=%E9%87%90%E6%B8%85), [Taiwan High Prosecutors Office](https://www.tph.moj.gov.tw/4421/4509/4515/1372878/post), [NTU course catalog](https://nol.ntu.edu.tw/nol/coursesearch/print_table.php?class=&course_id=104+12800&dpt_code=0000&lang=CH&semester=98-1&ser_no=10092), and [National Chengchi University repository](https://ah.lib.nccu.edu.tw/item?item_id=88411&locale=zh_TW). The NCCU [teaching assistant notes](https://selab.nccu.edu.tw/doku.php?id=tasop) attest 改作業.

The related headword 理清 uses attestations from the [PRC Ministry of Education](https://www.moe.gov.cn/jyb_sjzl/ziliao/A19/201807/W020180725662475781772.pdf), [Jinhu County](https://www.jinhu.gov.cn/col/1400_358822/art/o/17407584/1741078733013JQDx2gk5.html), and [Tangshan Normal University](https://xxzx.tsnu.edu.cn/info/1068/1721.htm).

## Build on Windows

Run from this directory with PowerShell 7, Rust stable, Python 3.13, and Visual Studio Build Tools. Python uses spaCy for dependency parsing, PyArrow for Parquet, and `jsonschema-rs` for full-archive schema checks. Rust handles scoring and packaging.

```powershell
pwsh -NoProfile -File .\scripts\fetch-inputs.ps1
py -3.13 -m venv .\inputs\python
& .\inputs\python\Scripts\python.exe -m pip install -r .\scripts\requirements.txt
& .\inputs\python\Scripts\python.exe .\scripts\extract-speech.py --workers 6
cargo run --release --locked
& .\inputs\python\Scripts\python.exe .\scripts\validate.py --audit .\inputs\audit.txt --preview .\inputs\preview.html
```

`inputs/sources.json` records download URLs and SHA-256 hashes. The validator checks all ZIP members against the Yomitan schemas, verifies internal link targets, and compares the complete entry sets and payloads across spelling variants. The audit lists a fixed set of common and confusable headwords for reading through the generated rankings.

## Publish an update

Run **Build and publish dictionary** from the repository’s Actions tab on `main` with a higher three-part revision. It builds and validates the dictionary, then publishes the ZIP and a byte-for-byte copy of its `index.json` as the latest release. These two asset filenames and the dictionary title are stable across releases so existing installs can update.
