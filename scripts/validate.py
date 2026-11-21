"""Validate all ZIP members and render a small popup preview from the generated entries."""

import argparse
from collections import Counter
from collections import defaultdict
from hashlib import sha256
from html import escape
import json
from pathlib import Path
import re
from urllib.parse import parse_qs, urlsplit
from zipfile import ZipFile

import jsonschema_rs


ROOT = Path(__file__).resolve().parents[1]
PROBES = {
    "釐清": ("釐清案情", "釐清責任", "釐清事實", "釐清定義", "釐清概念"),
    "厘清": ("釐清案情", "釐清責任", "釐清事實", "釐清定義", "釐清概念"),
    "澄清": ("澄清事實", "澄清傳聞"),
    "作業": ("寫作業", "做作業", "交作業", "改作業", "進行作業", "課業"),
    "作业": ("寫作業", "做作業", "交作業", "改作業", "進行作業", "課業"),
    "學習": ("補語", "學習到", "學習好", "學習完"),
    "学习": ("補語", "學習到", "學習好", "學習完"),
    "理清": ("理清思路", "理清頭緒"),
}
PREVIEW = ("釐清", "澄清", "作業", "作业")
PARITY_PROBES = (("釐清", "厘清"), ("作業", "作业"), ("學習", "学习"), ("计划", "計劃", "計畫", "计画"), ("发", "發", "髮"))
AUDIT = "釐清 澄清 理清 明白 懂 理解 知道 了解 認識 明確 發現 發覺 察覺 感覺 覺得 認為 以為 表示 表達 說明 解釋 掌握 把握 保持 維持 持續 繼續 遵守 遵循 執行 實施 完成 結束 解決 處理 對付 達到 實現 取得 獲得 獲取 提出 提供 提交 造成 產生 引起 導致 促進 推動 提高 提升 改善 改進 改變 變化 增加 增長 降低 減少 減輕 開展 進行 舉行 舉辦 做 寫 交 批改 作業 任務 學校 公司 工作 手機 電腦 新聞 消息 真相 事實 概念 責任 問題 答案 方案 計畫 決定 意見 建議 影響 風險 機會 情況 心情 時間 交通 費用 價格 合同 效果 飯".split()
HOLDOUT = "收集 搜集 採取 採用 接觸 接近 照顧 照料 照看 聘請 保護 挑戰 準備 咖啡 房子 窗戶 會議 約會 比賽 法律".split()


def nodes(value):
    if isinstance(value, dict):
        yield value
        yield from nodes(value.get("content"))
    elif isinstance(value, list):
        for item in value:
            yield from nodes(item)


def display_text(value):
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        return display_text(value.get("content", ""))
    return "".join(map(display_text, value))


def html(value):
    if isinstance(value, str):
        return escape(value)
    if isinstance(value, list):
        return "".join(map(html, value))
    tag = value["tag"]
    attributes = {f"data-sc-{key}": item for key, item in value.get("data", {}).items()}
    attributes.update({key: value[key] for key in ("lang", "href") if key in value})
    if value.get("style", {}).get("whiteSpace") == "nowrap":
        attributes["style"] = "white-space:nowrap"
    attributes = "".join(f' {key}="{escape(item, quote=True)}"' for key, item in attributes.items())
    return f"<{tag}{attributes}>{html(value['content'])}</{tag}>"


def preview(path, probes, css):
    shell_css = """body{margin:0;background:#777;font:16px/1.55 sans-serif}
main{display:grid;grid-template-columns:1fr 1fr;gap:1px}section{padding:18px;min-width:0}
.light{background:#fff;color:#242424}.dark{background:#202124;color:#e8e8e8}
article{padding:14px;margin-bottom:14px;border:1px solid #8887;border-radius:6px}
a{color:#1269ae}.dark a{color:#81aff3}h2{font-size:18px;margin:0 0 8px}small{font-size:14px;font-weight:400;margin-left:8px}"""
    cards = "".join(f"<article><h2>{word}<small>{escape(probes[word][1])}</small></h2>{html(probes[word][5][0]['content'])}</article>" for word in PREVIEW)
    path.write_text(f'<!doctype html><meta charset="utf-8"><style>{css}\n{shell_css}</style><main><section class="light">{cards}</section><section class="dark">{cards}</section></main>', encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?", default=ROOT / "dist/mandarin-collocations.zip")
    parser.add_argument("--inputs", type=Path, default=ROOT / "inputs")
    parser.add_argument("--preview", type=Path)
    parser.add_argument("--audit", type=Path)
    args = parser.parse_args()

    def validator(schema):
        return jsonschema_rs.validator_for(json.loads((args.inputs / schema).read_bytes()))

    index_validator = validator("dictionary-index-schema.json")
    term_validator = validator("dictionary-term-bank-v3-schema.json")
    entries, links, audit = set(), set(), {}
    probes, occurrences = {}, Counter()
    sequences_by_form = defaultdict(set)
    forms_by_sequence = defaultdict(set)
    payload_by_sequence = {}
    with ZipFile(args.archive) as archive:
        assert archive.testzip() is None, "ZIP failed its CRC check"
        names = archive.namelist()
        assert len(set(names)) == len(names), "Duplicate ZIP members"
        index_bytes = archive.read("index.json")
        assert index_bytes == args.archive.with_name("index.json").read_bytes(), "Release and embedded indexes differ"
        index = json.loads(index_bytes)
        index_validator.validate(index)
        assert index["sourceLanguage"] == "zh" and index["targetLanguage"] == "zh"
        assert re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", index["revision"])
        assert index["isUpdatable"] is True
        latest = index["url"] + "/releases/latest/download/"
        assert index["indexUrl"] == latest + "index.json"
        assert index["downloadUrl"] == latest + "mandarin-collocations.zip"
        css = archive.read("styles.css")
        assert css == (ROOT / "styles.css").read_bytes(), "Shared stylesheet was altered"
        banks = sorted(int(re.fullmatch(r"term_bank_(\d+)\.json", name)[1]) for name in names if name.startswith("term_bank_"))
        assert set(names) == {"index.json", "styles.css", *(f"term_bank_{i}.json" for i in banks)}
        assert banks and banks == list(range(1, len(banks) + 1))

        for number in banks:
            name = f"term_bank_{number}.json"
            bank = json.loads(archive.read(name))
            term_validator.validate(bank)
            for row in bank:
                word, _, _, _, _, glossary, sequence, _ = row
                assert (word, sequence) not in entries, f"Duplicate lookup row {word}"
                entries.add((word, sequence))
                occurrences[word] += 1
                sequences_by_form[word].add(sequence)
                forms_by_sequence[sequence].add(word)
                signature = sha256(json.dumps(row[1:], ensure_ascii=False, separators=(",", ":")).encode()).digest()
                assert payload_by_sequence.setdefault(sequence, signature) == signature, f"Script changed the entry at {word}"
                content = glossary[0]["content"]
                assert len(content["content"]) > 1, f"Empty entry {word}"
                for node in nodes(content):
                    assert node.get("tag") != "summary" or node.get("content") != "简体搭配", f"Simplified dropdown at {word}"
                    if "href" in node:
                        target = urlsplit(node["href"])
                        query = parse_qs(target.query, strict_parsing=True)
                        assert not (target.scheme or target.netloc or target.path or target.fragment) and set(query) == {"query"}
                        links.add(query["query"][0])
                if word in PROBES and content["lang"] == "zh-Hant":
                    probes[word] = row
                if word in PROBES and word not in probes:
                    probes[word] = row
                if word in AUDIT or word in HOLDOUT:
                    audit[word] = [f"{display_text(block['content'][0])}  {display_text(block['content'][1])}" for block in content["content"][1:] if block["tag"] == "div"]

    assert not (links - occurrences.keys()), f"Dead internal links {sorted(links - occurrences.keys())[:12]}"
    for forms in forms_by_sequence.values():
        expected = sequences_by_form[next(iter(forms))]
        assert all(sequences_by_form[form] == expected for form in forms), f"Incomplete lookup family: {sorted(forms)}"
    for forms in PARITY_PROBES:
        expected = sequences_by_form[forms[0]]
        assert expected and all(sequences_by_form[form] == expected for form in forms), f"Lookup probe lacks script parity: {forms}"
    for word, examples in PROBES.items():
        body = display_text(probes[word][5][0])
        assert all(example in body for example in examples), (word, body)
    assert "澄清湖" not in display_text(probes["澄清"][5][0])
    assert probes["釐清"][6] == probes["厘清"][6]
    assert probes["作業"][6] == probes["作业"][6]
    assert probes["釐清"][6] != probes["澄清"][6]
    if args.preview:
        preview(args.preview, probes, css.decode("utf-8"))
    if args.audit:
        args.audit.write_text("\n".join(f"{word}\n" + "\n".join(audit.get(word, ["[missing]"])) for word in AUDIT + HOLDOUT), encoding="utf-8")
    audited = len(audit.keys() & set(AUDIT))
    holdout = len(audit.keys() & set(HOLDOUT))
    print(f"Validated {len(entries):,} rows in {len(banks)} banks, {len(links):,} internal links, and {len(PROBES)} lookup probes. Audit coverage: {audited}/{len(AUDIT)}. Holdout coverage: {holdout}/{len(HOLDOUT)}.")


if __name__ == "__main__":
    main()
