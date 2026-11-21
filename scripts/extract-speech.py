"""Extract dependency pairs from the text-only MagicData-RAMC Parquet shards."""

import argparse
from collections import defaultdict
from pathlib import Path
import re

import pyarrow.parquet as parquet
import spacy


HAN = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff]{1,10}\Z")
TRANSCRIPT_MARKER = re.compile(r"\[[^]]*\]|<[^>]*>")
CLAUSE = re.compile(r"[。！？!?；;]+")
NOMINAL = {"NOUN", "VERB"}
PREDICATE = {"VERB", "ADJ"}


def clauses(paths):
    for path in paths:
        file = parquet.ParquetFile(path)
        for batch in file.iter_batches(columns=["__key__", "txt"]):
            for key, transcript in zip(*[column.to_pylist() for column in batch.columns], strict=True):
                if not key or not transcript:
                    continue
                conversation = Path(key).name
                for row in transcript.splitlines():
                    columns = row.split("\t", 3)
                    if len(columns) != 4 or columns[1] == "G00000000":
                        continue
                    text = TRANSCRIPT_MARKER.sub("", columns[3])
                    if "智慧语音采集" in text or text.startswith("爱数智慧语音"):
                        continue
                    for clause in CLAUSE.split(text):
                        clause = clause.strip(" ，、,\t")
                        if 4 <= len(clause) <= 150:
                            yield clause, conversation


def pairs(doc):
    for child in doc:
        parent = child.head
        if child == parent or not HAN.fullmatch(child.text) or not HAN.fullmatch(parent.text):
            continue
        dep = child.dep_
        if dep in {"dobj", "obj"} and parent.pos_ == "VERB" and child.pos_ in NOMINAL:
            yield "VO", parent.text, child.text
        elif dep in {"nsubj", "top"} and parent.pos_ in PREDICATE and child.pos_ == "NOUN":
            yield "SP", child.text, parent.text
        elif dep == "amod" and parent.pos_ == "NOUN" and child.pos_ == "ADJ":
            yield "AN", child.text, parent.text
        elif dep == "advmod" and parent.pos_ in PREDICATE and child.pos_ == "ADV":
            yield "AP", child.text, parent.text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, default=Path("inputs"))
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    files = [args.inputs / f"ramc-{split}.parquet" for split in ("train", "validation", "test")]
    pipeline = spacy.load("zh_core_web_md", exclude=["ner"])
    support = defaultdict(set)
    conversations = set()
    count = 0
    for doc, conversation in pipeline.pipe(clauses(files), as_tuples=True, batch_size=128, n_process=args.workers):
        conversations.add(conversation)
        count += 1
        for pair in pairs(doc):
            support[pair].add(conversation)
        if count % 10000 == 0:
            print(f"Parsed {count:,} utterances", flush=True)

    output = args.inputs / "speech.tsv"
    temporary = output.with_suffix(".partial")
    written = 0
    with temporary.open("w", encoding="utf-8", newline="\n") as result:
        result.write("relation\tleft\tright\tconversations\n")
        for (relation, left, right), evidence in sorted(support.items()):
            if len(evidence) >= 2:
                result.write(f"{relation}\t{left}\t{right}\t{len(evidence)}\n")
                written += 1
    temporary.replace(output)
    print(f"{count:,} utterances in {len(conversations):,} conversations, {written:,} pairs attested in 2+ conversations")


if __name__ == "__main__":
    main()
