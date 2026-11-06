#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Informa cuántos candidatos hay en la caché por fuente, tema y sección.

Cada artículo puede usarse una sola vez, y el mismo título en dos wikis cuenta como un artículo.
El reparto entre secciones es una estimación por lo bajo: si alcanza, hay candidatos suficientes.

Uso:
    python3 tools/progression/report.py [--need N] [--cache CARPETA]

Escribe <caché>/report.md y termina con error si alguna sección no llega a N candidatos.
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
from pathlib import Path

from fetch import HERE, read_lines
from textrules import sections, sections_for, valid_cuts

NEEDED_PER_SECTION = 60  # El doble de las 30 lecciones, para poder descartar en la revisión.


def load_candidates(cache: Path, wikis: list[str], selected: dict) -> list[dict]:
    """Un registro por título y wiki con artículo y al menos un recorte válido."""
    out = []
    for wiki in wikis:
        for line in read_lines(cache / "articles" / f"{wiki}.jsonl"):
            article = line["article"]
            if not article:
                continue
            fits = sections_for(valid_cuts(article["extract"], article["title"]))
            if not fits:
                continue
            for seed in line["requested"]:
                if seed in selected:
                    out.append(
                        {"seed": seed, "wiki": wiki, "fits": fits, "topic": selected[seed]["topic"]}
                    )
    return out


def allocate(candidates: list[dict], need: int) -> dict[int, int]:
    """Reparte artículos entre secciones, empezando por las de menos candidatos."""
    by_seed: dict[str, set[int]] = collections.defaultdict(set)
    for candidate in candidates:
        by_seed[candidate["seed"]] |= candidate["fits"]
    count = {s: sum(s in fits for fits in by_seed.values()) for s in sections()}
    given = {s: 0 for s in sections()}
    free = dict(by_seed)
    for section in sorted(sections(), key=count.__getitem__):
        options = sorted(
            (seed for seed, fits in free.items() if section in fits),
            key=lambda seed: (len(free[seed]), seed),
        )
        for seed in options[:need]:
            del free[seed]
            given[section] += 1
    return given


def report(cache: Path, wikis: list[str], need: int) -> tuple[str, bool]:
    """Texto del informe y si todas las secciones llegan a lo necesario."""
    selected = json.loads((cache / "selected.json").read_text(encoding="utf-8"))
    candidates = load_candidates(cache, wikis, selected)
    given = allocate(candidates, need)
    out = ["# Candidatos para la Progresión", "", f"Títulos en la lista: {len(selected)}", ""]
    out += ["| Fuente | Artículos | Con recorte válido |", "|---|---|---|"]
    for wiki in wikis:
        articles = sum(
            1 for line in read_lines(cache / "articles" / f"{wiki}.jsonl") if line["article"]
        )
        usable = sum(1 for c in candidates if c["wiki"] == wiki)
        out.append(f"| {wiki} | {articles} | {usable} |")
    out += ["", "| Tema | Candidatos |", "|---|---|"]
    topics = collections.Counter(c["topic"] for c in candidates)
    out += [f"| {topic or '(sin tema)'} | {n} |" for topic, n in topics.most_common()]
    header = " | ".join(wikis)
    out += ["", f"| Sección | {header} | Reparto |", "|---|" + "---|" * (len(wikis) + 1)]
    for section in sections():
        counts = [
            str(sum(c["wiki"] == w and section in c["fits"] for c in candidates)) for w in wikis
        ]
        out.append(f"| {section} | {' | '.join(counts)} | {given[section]} |")
    enough = all(n >= need for n in given.values())
    verdict = "alcanza" if enough else "NO alcanza"
    out += ["", f"Control ({need} por sección): {verdict}.", ""]
    return "\n".join(out), enough


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--need", type=int, default=NEEDED_PER_SECTION)
    parser.add_argument("--cache", type=Path, default=HERE / "cache")
    args = parser.parse_args()
    sources = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))
    text, enough = report(args.cache, sources["wikis"], args.need)
    (args.cache / "report.md").write_text(text, encoding="utf-8")
    print(text)
    return 0 if enough else 1


if __name__ == "__main__":
    sys.exit(main())
