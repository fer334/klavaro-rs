# SPDX-License-Identifier: GPL-3.0-or-later
"""Reglas de los textos de la Progresión: limpieza, recorte, filtros y largo por sección."""

from __future__ import annotations

import re

FIRST_SECTION = 20
LAST_SECTION = 100
SECTION_STEP = 5
LESSONS = 30
LENGTH_TOLERANCE = 0.15
MAX_WORDS_IN_SENTENCE = 34
MAX_CAPITALS = 0.07
MAX_PUNCTUATION = 0.05
MAX_DIGITS = 0.04
MAX_PARENTHESES = 2
# Temas que no conviene hacer copiar en un ejercicio de mecanografía.
SENSITIVE = re.compile(
    r"\b(rape|raped|sexual\w*|sex|porn\w*|genocide|holocaust|suicide|murder\w*|massacre\w*|"
    r"nazi\w*|terror\w*|torture\w*|slave\w*|abortion|incest|pedophil\w*|paedophil\w*|"
    r"prostitut\w*|cocaine|heroin|lynch\w*|racis\w*|execut(?:ed|ion)|war crimes?|atrocit\w*|"
    r"assassinat\w*|mass shooting|extermin\w*|penis|vagina|genital\w*|bomb(?:ing|ings)|"
    r"atomic bomb\w*|nuclear weapons?|killed|dictator\w*|fascis\w*|rapist)\b",
    re.IGNORECASE,
)
# Frases que se desactualizan o que muestran que el texto no se sostiene solo.
UNSUITABLE = re.compile(
    r"\b(as of|currently|at present|recent years|this article|see also|is a list of|refers? to|"
    r"pronounced|listen|disambiguation|below|above|following:|template:|for the [^.]*, see|US:|20[12]\d)\b",
    re.IGNORECASE,
)
# Temas académicos o especializados, que se leen peor que uno cotidiano.
TECHNICAL = re.compile(
    r"\b(sub-?disciplines?|subfields?|branch of (?:mathematics|physics|chemistry|algebra|geometry|"
    r"statistics|topology)|in (?:mathematics|physics|chemistry|geometry|abstract algebra|"
    r"mathematical logic|statistics|nuclear physics|linguistics|taxonomy)\b|theorems?|polynomials?|"
    r"tensors?|scalars?|algebraic|prokaryot\w*|eukaryot\w*|macromolecul\w*|isotopes?|phylum|"
    r"subfamil\w*|superfamil\w*|clades?|principalit\w*|khanates?|sultanates?|caliphates?|"
    r"thalassocratic|polity|satraps?|historiograph\w*|hypothesis test\w*)\b",
    re.IGNORECASE,
)
LEFTOVERS = re.compile(r"''|\(\s|\)\(|\(\s*\)|\(\s*[,;:]|[,;:]\s*\)|,\s*,|\s{2}|\.\.|\s-\s*\)|\{|displaystyle")

ABBREVIATIONS = {
    "st", "dr", "mr", "mrs", "ms", "prof", "jr", "sr", "inc", "ltd", "co", "corp", "vs", "etc",
    "e.g", "i.e", "u.s", "u.k", "u.n", "c", "ca", "no", "mt", "ft", "gen", "col", "lt", "sgt",
    "rev", "gov", "sen", "rep", "pres", "approx", "jan", "feb", "mar", "apr", "jun", "jul", "aug",
    "sep", "sept", "oct", "nov", "dec",
}
TYPOGRAPHIC = str.maketrans(
    {
        "‘": "'", "’": "'", "“": '"', "”": '"', "–": "-", "—": "-",
        "−": "-", " ": " ", " ": " ", "​": "",
    }
)
ALLOWED = re.compile(r"[A-Za-z0-9 .,'\"\-:;!?()]*")
PARENTHESIS = re.compile(r"\s*\([^()]*\)")
NON_ASCII = re.compile(r"[^\x00-\x7f]")


def section_chars(section: int) -> tuple[float, float]:
    """Largo mínimo y máximo de los textos de una sección, de la lección 1 a la 30."""
    first = section - SECTION_STEP + SECTION_STEP / LESSONS
    return first * 5 * (1 - LENGTH_TOLERANCE), section * 5 * (1 + LENGTH_TOLERANCE)


def sections() -> list[int]:
    """Las secciones, de la primera a la última."""
    return list(range(FIRST_SECTION, LAST_SECTION + 1, SECTION_STEP))


def clean(extract: str) -> str:
    """Normaliza la tipografía, quita los paréntesis con pronunciación o letras de otros
    alfabetos y deja todo en una línea."""

    def keep_or_drop(match: re.Match[str]) -> str:
        inside = match.group(0)
        # Sin minúsculas ni dígitos sólo quedan restos de la pronunciación: «(, US: )».
        useless = not re.search(r"[a-z0-9]", inside)
        return "" if useless or NON_ASCII.search(inside) else inside

    text = extract.translate(TYPOGRAPHIC)
    text = PARENTHESIS.sub(keep_or_drop, text)
    text = re.sub(r"\s+", " ", text).strip()
    return re.sub(r"\s+([.,;:])", r"\1", text)


def split_sentences(text: str) -> list[str]:
    """Separa oraciones sin cortar en abreviaturas ni iniciales."""
    pieces = re.split(r'(?<=[.!?])["\')]?\s+(?=["\'(]?[A-Z0-9])', text)
    sentences: list[str] = []
    for piece in pieces:
        if sentences and is_unfinished(sentences[-1]):
            sentences[-1] += " " + piece
        else:
            sentences.append(piece)
    return sentences


def is_unfinished(sentence: str) -> bool:
    """Si la oración termina en una abreviatura o inicial y por eso sigue en la próxima parte."""
    last = sentence.rsplit(" ", 1)[-1].rstrip(".").lstrip("(\"'").lower()
    return len(last) == 1 or last in ABBREVIATIONS


def topic_word(title: str) -> str:
    """Palabra del título que se espera en la primera oración."""
    plain = re.sub(r"\s*\(.*\)$", "", title)
    words = [w for w in re.findall(r"[A-Za-z0-9]+", plain) if w.lower() not in {"the", "a", "an", "of"}]
    return words[0].lower() if words else ""


def is_valid(text: str, title: str) -> bool:
    """Si un recorte cumple los filtros tomados del análisis de EdClub."""
    if not text.endswith(".") or not ALLOWED.fullmatch(text):
        return False
    if "may refer to" in text or "(see " in text or text.count("(") != text.count(")") or text.count('"') % 2:
        return False
    if text.count("(") > MAX_PARENTHESES or LEFTOVERS.search(text):
        return False
    if SENSITIVE.search(text) or UNSUITABLE.search(text) or TECHNICAL.search(text):
        return False
    if not text[0].isupper() and text[0] not in "\"'":
        return False
    size = len(text)
    if sum(c.isupper() for c in text) / size > MAX_CAPITALS:
        return False
    if sum(not c.isalnum() and not c.isspace() for c in text) / size > MAX_PUNCTUATION:
        return False
    if sum(c.isdigit() for c in text) / size > MAX_DIGITS:
        return False
    sentences = split_sentences(text)
    if max(len(s.split()) for s in sentences) > MAX_WORDS_IN_SENTENCE:
        return False
    return topic_word(title) in sentences[0].lower()


def valid_cuts(extract: str, title: str) -> list[str]:
    """Recortes válidos desde el inicio del artículo, cortando en oraciones completas."""
    cuts = []
    text = ""
    for sentence in split_sentences(clean(extract)):
        text = f"{text} {sentence}".strip()
        if len(text) > section_chars(LAST_SECTION)[1]:
            break
        if is_valid(text, title):
            cuts.append(text)
    return cuts


def sections_for(cuts: list[str]) -> set[int]:
    """Secciones en las que entra algún recorte."""
    fits = set()
    for section in sections():
        low, high = section_chars(section)
        if any(low <= len(cut) <= high for cut in cuts):
            fits.add(section)
    return fits
