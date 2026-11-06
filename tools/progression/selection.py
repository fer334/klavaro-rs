#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Elige de la caché los textos de cada lección de la Progresión y arma el catálogo.

Cada texto recibe un índice de dificultad D de 0 a 100. Las secciones toman textos de una franja
de D que sube con la meta y cada lección pide el largo que dura unos 60 s a su meta. Los textos
van a una revisión: review.json (versionado) fija los aprobados en su lección y guarda los
artículos rechazados, así las decisiones sobreviven a una nueva generación. Una lección sin texto
aprobado toma el siguiente candidato y sólo hay que revisar ese.

Uso:
    python3 tools/progression/selection.py             propone y escribe <caché>/proposal.tsv
    python3 tools/progression/selection.py --pending   muestra los textos sin revisar
    python3 tools/progression/selection.py --approve  aprueba la propuesta (menos lo rechazado)
    python3 tools/progression/selection.py --write     escribe assets/C.progression (todo aprobado)
"""

from __future__ import annotations

import argparse
import bisect
import collections
import hashlib
import json
import math
import re
import sys
from pathlib import Path

from fetch import HERE, read_lines
from textrules import LENGTH_TOLERANCE, LESSONS, SECTION_STEP, sections, split_sentences, valid_cuts

CATALOG = HERE.parent.parent / "assets" / "C.progression"
REVIEW = HERE / "review.json"
HEADER = """\
# Textos de la Progresión, de los colaboradores de Wikipedia y Simple English Wikipedia.
# Licencia: Creative Commons Attribution-ShareAlike 4.0
# (https://creativecommons.org/licenses/by-sa/4.0/). Recortados y normalizados.
# Enlace a cada original: https://<fuente>.wikipedia.org/w/index.php?oldid=<revisión>
# sección\tlección\ttext_id\ttema\tD\tfuente\ttítulo\trevisión\ttexto
"""
D_LOW = 10.0
D_HIGH = 90.0
D_WIDTH = 15.0
# Se prefieren los temas cotidianos: hay un tope por sección para los más académicos.
MAX_PER_TOPIC = 7
TOPIC_LIMITS = {
    "Mathematics": 1,
    "Physical sciences": 3,
    "History": 3,
    "People": 3,
    "Philosophy and religion": 3,
}
MAX_RELIGIOUS = 1  # Textos sobre religión o dioses por sección, sea cual sea su tema.
RELIGIOUS = re.compile(
    r"\b(religio\w*|gods?|goddess\w*|deit\w*|divin\w*|church\w*|basilica|cathedral|bible|biblical|"
    r"christ|christian\w*|catholic\w*|protestant\w*|pope|bishop|saint|nun|priest\w*|monk|theolog\w*|islam\w*|"
    r"muslim\w*|quran|hindu\w*|buddh\w*|jews?|jewish|juda\w*|torah|hebrew|prophet\w*|scriptures?|"
    r"sacred|holy|spirit\w*|soul|prayer|pilgrim\w*|mytholog\w*|myths?|shinto\w*|tao\w*|confuci\w*|"
    r"zen|demons?|demonic|commandments?|shaman\w*|zoroastr\w*|vedas?|veda|sect)\b",
    re.IGNORECASE,
)
MIN_VIEWS = 150  # Visitas diarias mínimas del artículo: deja afuera los temas muy especializados.
MAX_SHARED_WORDS = 0.5
WEIGHTS = {"rare": 0.45, "word": 0.2, "sentence": 0.2, "capitals": 0.05, "symbols": 0.1}


def words_of(text: str) -> list[str]:
    return re.findall(r"[a-z']+", text.lower())


def target_wpm(section: int, lesson: int) -> float:
    return section - SECTION_STEP + lesson * SECTION_STEP / LESSONS


def target_chars(section: int, lesson: int) -> float:
    return target_wpm(section, lesson) * 5


def section_center(section: int) -> float:
    """Centro de la franja de dificultad de una sección."""
    all_sections = sections()
    return D_LOW + (D_HIGH - D_LOW) * all_sections.index(section) / (len(all_sections) - 1)


def features(text: str, frequency: dict[str, float]) -> dict[str, float]:
    """Rasgos de dificultad que no dependen del teclado."""
    words = words_of(text) or [""]
    size = len(text)
    rarity = [frequency.get(w, 0.0) for w in words]
    return {
        "rare": sum(rarity) / len(rarity),
        "word": sum(map(len, words)) / len(words),
        "sentence": len(text.split()) / len(split_sentences(text)),
        "capitals": sum(c.isupper() for c in text) / size,
        "symbols": sum(not c.isalpha() and not c.isspace() for c in text) / size,
    }


def word_rarity(extracts: list[str]) -> dict[str, float]:
    """Rareza de cada palabra en el corpus: cuanto menos artículos la usan, mayor."""
    documents: collections.Counter[str] = collections.Counter()
    for extract in extracts:
        documents.update(set(words_of(extract)))
    total = max(len(extracts), 1)
    return {word: math.log10(total / count) for word, count in documents.items()}


class Difficulty:
    """Calcula D como la mezcla de los percentiles de cada rasgo en el corpus."""

    def __init__(self, samples: list[dict[str, float]]) -> None:
        self.sorted = {name: sorted(s[name] for s in samples) for name in WEIGHTS}

    def __call__(self, values: dict[str, float]) -> float:
        score = 0.0
        for name, weight in WEIGHTS.items():
            column = self.sorted[name]
            score += weight * bisect.bisect_left(column, values[name]) / max(len(column), 1)
        return round(100 * score, 1)


def stable_noise(*parts: object) -> int:
    """Variación repetible entre ejecuciones, para no elegir siempre artículos parecidos."""
    digest = hashlib.sha256("|".join(map(str, parts)).encode()).digest()
    return digest[0] % 16


def is_religious(cut: dict) -> bool:
    """Si el texto trata de religión: se mira el título y la primera oración."""
    return bool(RELIGIOUS.search(f"{cut['title']} {split_sentences(cut['text'])[0]}"))


def topic_limit(topic: str) -> int:
    return TOPIC_LIMITS.get(topic, MAX_PER_TOPIC)


def load_candidates(cache: Path, wikis: list[str], selected: dict) -> list[dict]:
    """Todos los recortes válidos, con su artículo, sus visitas y su dificultad."""
    views = json.loads((cache / "views.json").read_text(encoding="utf-8"))
    articles = []
    for wiki in wikis:
        for line in read_lines(cache / "articles" / f"{wiki}.jsonl"):
            seeds = [s for s in line["requested"] if s in selected]
            if line["article"] and seeds:
                articles.append((line["article"], seeds))
    frequency = word_rarity([a["extract"] for a, _ in articles])
    cuts = []
    for article, seeds in articles:
        for text in valid_cuts(article["extract"], article["title"]):
            cuts.append(
                {
                    "id": f"{article['wiki']}-{article['pageid']}",
                    "wiki": article["wiki"],
                    "title": article["title"],
                    "revision": article["revision"],
                    "seed": seeds[0].lower(),
                    "topic": selected[seeds[0]]["topic"],
                    "level": selected[seeds[0]]["level"],
                    "views": views.get(seeds[0], 0),
                    "text": text,
                    "values": features(text, frequency),
                }
            )
    difficulty = Difficulty([cut["values"] for cut in cuts])
    for cut in cuts:
        cut["d"] = difficulty(cut["values"])
    cuts.sort(key=lambda c: (c["id"], len(c["text"])))
    return cuts


def shared_words(a: str, b: str) -> float:
    first, second = set(words_of(a)), set(words_of(b))
    return len(first & second) / max(min(len(first), len(second)), 1)


def is_usable(cut: dict, rejected: set[str]) -> bool:
    return cut["id"] not in rejected and cut.get("views", MIN_VIEWS) >= MIN_VIEWS


def pinned_rows(cuts: list[dict], rejected: set[str], approved: dict[str, dict]) -> dict[tuple[int, int], dict]:
    """Textos aprobados que siguen disponibles, por lección."""
    by_key = {(c["id"], fingerprint(c)): c for c in cuts if is_usable(c, rejected)}
    pinned = {}
    for slot, choice in approved.items():
        cut = by_key.get((choice["id"], choice["fingerprint"]))
        if cut:
            section, lesson = map(int, slot.split("."))
            pinned[(section, lesson)] = {**cut, "section": section, "lesson": lesson}
    return pinned


def assign(
    cuts: list[dict], rejected: set[str], approved: dict[str, dict] | None = None
) -> tuple[list[dict], list[tuple[int, int]]]:
    """Elige un texto para cada lección, respetando los aprobados. Devuelve las filas y las
    lecciones sin cubrir."""
    usable = [c for c in cuts if is_usable(c, rejected)]
    pinned = pinned_rows(cuts, rejected, approved or {})
    rows: list[dict] = []
    missing: list[tuple[int, int]] = []
    used_seeds: set[str] = {row["seed"] for row in pinned.values()}
    # Dos wikis pueden llegar al mismo tema desde títulos distintos de la lista.
    used_titles: set[str] = {row["title"].lower() for row in pinned.values()}
    used_texts: list[str] = [row["text"] for row in pinned.values()]
    for section in sections():
        center = section_center(section)
        pool = [c for c in usable if abs(c["d"] - center) <= D_WIDTH]
        topics: collections.Counter[str] = collections.Counter(
            row["topic"] for (s, _), row in pinned.items() if s == section
        )
        religious = sum(is_religious(r) for (s, _), r in pinned.items() if s == section)
        previous = ""
        for lesson in range(1, LESSONS + 1):
            fixed = pinned.get((section, lesson))
            if fixed:
                rows.append(fixed)
                previous = fixed["topic"]
                continue
            following = pinned.get((section, lesson + 1), {}).get("topic", "")
            target = target_chars(section, lesson)
            low, high = target * (1 - LENGTH_TOLERANCE), target * (1 + LENGTH_TOLERANCE)

            def cost(cut: dict, section: int = section, lesson: int = lesson) -> tuple:
                return (
                    abs(len(cut["text"]) - target) / target * 40
                    + abs(cut["d"] - center)
                    + (cut["level"] - 1) * 6
                    - math.log10(max(cut.get("views", MIN_VIEWS), 1)) * 8
                    + topics[cut["topic"]] * 5
                    + (30 if cut["topic"] == "Mathematics" else 0)
                    + stable_noise(section, lesson, cut["id"]),
                    cut["id"],
                    len(cut["text"]),
                )

            options = sorted(
                (
                    c
                    for c in pool
                    if low <= len(c["text"]) <= high
                    and c["seed"] not in used_seeds
                    and c["title"].lower() not in used_titles
                    and c["topic"] != previous
                    and c["topic"] != following
                    and topics[c["topic"]] < topic_limit(c["topic"])
                    and (religious < MAX_RELIGIOUS or not is_religious(c))
                ),
                key=cost,
            )
            chosen = next(
                (
                    c
                    for c in options
                    if all(shared_words(c["text"], t) <= MAX_SHARED_WORDS for t in used_texts)
                ),
                None,
            )
            if chosen is None:
                missing.append((section, lesson))
                continue
            rows.append({**chosen, "section": section, "lesson": lesson})
            used_seeds.add(chosen["seed"])
            used_titles.add(chosen["title"].lower())
            used_texts.append(chosen["text"])
            topics[chosen["topic"]] += 1
            religious += is_religious(chosen)
            previous = chosen["topic"]
    return rows, missing


def fingerprint(row: dict) -> str:
    """Identifica el texto aprobado: si el texto cambia, hay que revisarlo otra vez."""
    return hashlib.sha256(row["text"].encode()).hexdigest()[:12]


def load_review(path: Path) -> dict:
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    return {"approved": {}, "rejected": {}}


def slot_of(row: dict) -> str:
    return f"{row['section']}.{row['lesson']}"


def pending_rows(rows: list[dict], review: dict) -> list[dict]:
    """Filas que no son exactamente el texto aprobado para su lección."""
    return [
        r
        for r in rows
        if review["approved"].get(slot_of(r))
        != {"id": r["id"], "fingerprint": fingerprint(r)}
    ]


def approve(rows: list[dict], review: dict) -> dict:
    """Fija en su lección las filas cuyo artículo no se rechazó."""
    approved = dict(review["approved"])
    for row in rows:
        if row["id"] not in review["rejected"]:
            approved[slot_of(row)] = {"id": row["id"], "fingerprint": fingerprint(row)}
    return {**review, "approved": dict(sorted(approved.items(), key=slot_order))}


def read_proposal(path: Path) -> list[dict]:
    """Filas de una propuesta ya escrita, con lo necesario para aprobarlas."""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("#"):
            fields = line.split("\t")
            rows.append(
                {"section": int(fields[0]), "lesson": int(fields[1]), "id": fields[2], "text": fields[8]}
            )
    return rows


def slot_order(item: tuple[str, dict]) -> tuple[int, int]:
    section, lesson = item[0].split(".")
    return int(section), int(lesson)


def row_line(row: dict) -> str:
    fields = (
        row["section"], row["lesson"], row["id"], row["topic"], f"{row['d']:.1f}", row["wiki"],
        row["title"], row["revision"], row["text"],
    )
    return "\t".join(str(field) for field in fields)


def catalog_text(rows: list[dict]) -> str:
    return HEADER + "".join(row_line(row) + "\n" for row in rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cache", type=Path, default=HERE / "cache")
    parser.add_argument("--pending", action="store_true", help="muestra los textos sin revisar")
    parser.add_argument("--approve", action="store_true", help="aprueba la propuesta")
    parser.add_argument("--write", action="store_true", help="escribe el catálogo")
    args = parser.parse_args()
    sources = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))
    selected = json.loads((args.cache / "selected.json").read_text(encoding="utf-8"))
    review = load_review(REVIEW)
    cuts = load_candidates(args.cache, sources["wikis"], selected)
    if args.approve:
        # Se aprueba lo que se mostró en la última propuesta, nunca un reemplazo sin leer.
        review = approve(read_proposal(args.cache / "proposal.tsv"), review)
        REVIEW.write_text(json.dumps(review, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    rows, missing = assign(cuts, set(review["rejected"]), review["approved"])
    (args.cache / "proposal.tsv").write_text(catalog_text(rows), encoding="utf-8")
    pending = pending_rows(rows, review)
    print(f"{len(rows)} lecciones cubiertas, {len(missing)} sin cubrir, {len(pending)} sin revisar")
    if missing:
        print("sin cubrir:", missing)
    if args.pending:
        for row in pending:
            print(f"{row['section']}.{row['lesson']} {row['id']} [{row['topic']}] D{row['d']:.0f} {row['text']}")
    if args.write:
        if missing or pending:
            print("error: hay lecciones sin cubrir o sin revisar", file=sys.stderr)
            return 1
        CATALOG.write_text(catalog_text(rows), encoding="utf-8")
        print(f"escrito {CATALOG}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
