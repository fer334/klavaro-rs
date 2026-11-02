#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Convierte las traducciones del Klavaro original (po/*.po) a Fluent (.ftl).

Sólo se toman los mensajes de la tabla MESSAGES, que la interfaz de klavaro-rs reutiliza tal
cual. Los textos propios de klavaro-rs viven en crates/klavaro-i18n/locales/app/.

Uso:
    python3 tools/po2ftl.py [upstream/klavaro-3.14/po]

Escribe crates/klavaro-i18n/locales/klavaro/<código>.ftl (C.ftl con los textos en inglés del
original) y crates/klavaro-i18n/src/catalog.rs. Termina con error si algún msgid de la tabla no
está en klavaro.pot.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "crates/klavaro-i18n/locales/klavaro"
CATALOG = ROOT / "crates/klavaro-i18n/src/catalog.rs"

# (id de Fluent, msgid del original, nombres de los argumentos en orden de aparición)
MESSAGES = [
    ("module-intro", "Introduction", []),
    ("module-basic", "Basic course", []),
    ("module-adaptability", "Adaptability", []),
    ("module-velocity", "Speed", []),
    ("module-fluidity", "Fluidity", []),
    ("module-basic-tip", "First steps with initial training", []),
    ("module-adaptability-tip", "Practice with random keys", []),
    ("module-velocity-tip", "Practice with random words", []),
    ("module-fluidity-tip", "Practice with complete texts", []),
    ("app-tagline", "A very flexible and efficient touch typing tutor.", []),
    ("app-motto", "Remember always: someone loves you!", []),
    ("menu-keyboard", "_Keyboard:", []),
    ("menu-language", "_Language:", []),
    ("menu-text-language-tip", "Language for modules 3 and 4.", []),
    ("menu-back", "B_ack to menu", []),
    ("menu-top10", "Top 10", []),
    ("menu-progress", "_Progress", []),
    ("tutor-restart-tip", "Press here to restart the exercise. Hotkey: [Ctrl-R]", []),
    ("tutor-lesson", "Lesson:", []),
    ("tutor-paragraphs", "Paragraphs:", []),
    ("tutor-keys", "Keys:", []),
    ("tutor-velocity-heading", "Velocity exercises: accelerate typing real words.", []),
    ("tutor-fluidity-heading", "Fluidness exercises: accuracy typing good sense paragraphs.", []),
    ("tutor-end", "End of exercise. Press [Enter] to start another.", []),
    ("tutor-start", "Start typing when you are ready. ", []),
    ("tutor-use-backspace", "Use backspace to correct errors.", []),
    ("tutor-key-positions-learned", "Positions of keys seems to be learned!", []),
    ("stats-title", "STATISTICS", []),
    ("stats-elapsed", "Elapsed time:", []),
    ("stats-error-ratio", "Error ratio:", []),
    ("stats-accuracy", "Accuracy:", []),
    ("stats-goal", "Goal:", []),
    ("stats-cps", "Characters per second:", []),
    ("stats-wpm", "Words per minute:", []),
    ("stats-fluidity", "Fluidness:", []),
    ("stats-comments", "Comments:", []),
    ("stats-score", "Score", []),
    ("progress-accuracy", "Accuracy", []),
    ("progress-speed", "Speed", []),
    ("progress-fluidity", "Fluidity", []),
    ("progress-wpm-unit", "(WPM)", []),
    ("progress-date", "Date & Time", []),
    ("progress-errors", "Errors", []),
    ("progress-touch-times", "Touch times (s)", []),
    ("progress-character", "Character", []),
    ("tutor-special-tip", "This is a specific exercise, adjusted to your error profile.", []),
    ("menu-speech", "Speech", []),
    ("speech-voice", "en", []),
    ("speech-space", "space", []),
    ("speech-wye", "wye", []),
    ("speech-enter", "enter", []),
    ("speech-apostrophe", "apostrophe", []),
    ("speech-quote", "quote", []),
    ("speech-ampersand", "ampersand", []),
    ("ranking-name", "Name", []),
    ("ranking-chars", "Chars", []),
    ("ranking-when", "When", []),
    ("ranking-local", "Local scores", []),
    ("ranking-empty", "Empty ranking. Please practice fluidness.", []),
    ("ps-ranked", "ps.: you have entered the Top 10 list, great!", []),
    (
        "ps-not-logged",
        "ps.: logging not performed for this session: the number of typed characters (%i) "
        "must be greater than %i.",
        ["chars", "min"],
    ),
    (
        "ps-not-similar",
        "ps.: the text you just typed doesn't seem to be similar to ordinary texts in the "
        "language currently selected: we can't account for it in the 'Top 10' contest.",
        [],
    ),
    (
        "comment-basic-next",
        " All right, now you got it!\n Go to the next lesson.\n",
        [],
    ),
    (
        "comment-basic-done",
        " Congratulations!\n"
        " You have accomplished the entire basic course.\n"
        " Go to the next type of exercise: adaptability.\n"
        " There you will practice mainly the accuracy.\n",
        [],
    ),
    (
        "comment-adapt-try-again",
        " Your accuracy rate is below %.0f%%...\n"
        " Could you please try again to improve it?\n",
        ["target"],
    ),
    (
        "comment-adapt-improving",
        " You are doing well. But...\n Could you make the accuracy reach %.0f%%?\n",
        ["target"],
    ),
    (
        "comment-adapt-almost",
        " You are almost there, but your accuracy rate is still below %.0f%%.\n"
        " Try a few more times, or maybe you're getting upset, so go to another kind of "
        "exercise.\n",
        ["target"],
    ),
    (
        "comment-adapt-done",
        " Very good!\n"
        " You succeeded with an accuracy rate above %.0f%%.\n"
        " Now it is time to increase your velocity.\n"
        " Go to the 3rd exercise at the main menu.\n",
        ["target"],
    ),
    (
        "comment-velo-beginning",
        " You are just beginning.\n"
        " Be patient, try it again every day, rest and don't worry so much:\n"
        " persistence and practice will improve your velocity.\n",
        [],
    ),
    (
        "comment-velo-highway",
        " Still away from the highway. You can do better...\n Try to reach at least %.0f WPM.\n",
        ["target"],
    ),
    (
        "comment-velo-faster",
        " You are doing well, but need to go faster.\n"
        " And don't forget the accuracy. Try to get %.0f WPM.\n",
        ["target"],
    ),
    (
        "comment-velo-running",
        " Fine. Now you need to start running.\n Can you reach %.0f WPM?\n",
        ["target"],
    ),
    (
        "comment-velo-almost",
        " Very good. You are almost there.\n Can you finally reach %.0f WPM?\n",
        ["target"],
    ),
    (
        "comment-velo-enough",
        " Excellent. For this course, that is enough.\n Try now the fluidness exercises, OK?\n",
        [],
    ),
    (
        "comment-velo-professional",
        " Fast! Are you a professional?\n So, try to get %.0f WPM!\n",
        ["target"],
    ),
    (
        "comment-velo-speed-racer",
        " Ranking good, Speed Racer! Are you afraid of reaching %.0f WPM?\n",
        ["target"],
    ),
    (
        "comment-velo-kung-fu",
        " Kung-fu mastery!\n Can you fly at %.0f WPM?\n",
        ["target"],
    ),
    (
        "comment-velo-master",
        " Dvorak master!\n I have no words to express my admiration!\n",
        [],
    ),
    (
        "comment-fluid-too-slow",
        " You type accurately but not so fast.\n Can you reach %.0f WPM?\n",
        ["target"],
    ),
    (
        "comment-fluid-irregular",
        " Your rhythm is not so constant. Calm down.\n"
        " For now, try to make the fluidness greater than %i%%.\n",
        ["target"],
    ),
    (
        "comment-fluid-almost",
        " You are almost getting there. Type more fluently.\n"
        " I want a fluidness greater than %.0f%%.\n",
        ["target"],
    ),
    (
        "comment-fluid-congratulations",
        " Congratulations!\n"
        " It seems to me that you are a professional.\n"
        " You don't need this program (me) anymore.\n"
        " Hope you have enjoyed. Thanks and be happy!\n",
        [],
    ),
    (
        "comment-fluid-beyond",
        " How can you type so fast?\n"
        " You have exceeded all my expectations.\n"
        " Are you a machine? Could you teach me?\n"
        " I can not help you anymore. Go to an expert!\n",
        [],
    ),
]

C_ESCAPES = {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}


def unquote(s):
    """Contenido de una cadena C entre comillas."""
    s = s.strip()
    assert s.startswith('"') and s.endswith('"'), s
    body = s[1:-1]
    return re.sub(r"\\(.)", lambda m: C_ESCAPES.get(m.group(1), m.group(1)), body)


def parse_po(path):
    """Diccionario msgid -> msgstr de las entradas traducidas y no difusas (sin plurales)."""
    entries = {}
    fuzzy = False
    field = None
    cur = {}

    def flush():
        nonlocal cur, fuzzy
        msgid = cur.get("msgid")
        msgstr = cur.get("msgstr")
        if msgid and msgstr and not fuzzy and "msgid_plural" not in cur:
            entries[msgid] = msgstr
        cur = {}
        fuzzy = False

    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            flush()
            field = None
            continue
        if line.startswith("#~"):
            continue
        if line.startswith("#,"):
            fuzzy = "fuzzy" in line
            continue
        if line.startswith("#"):
            continue
        m = re.match(r"^(msgid|msgid_plural|msgstr(?:\[\d+\])?)\s+(\".*\")$", line)
        if m:
            field = m.group(1)
            cur[field] = unquote(m.group(2))
        elif line.startswith('"') and field:
            cur[field] += unquote(line)
    flush()
    return entries


def printf_to_fluent(text, args, ftl_id):
    """Cambia los `%` de printf por `{ $arg }`; `%%` queda como `%`.

    Acepta argumentos con posición (`%2$i`). Devuelve None si la cantidad no coincide.
    """
    out = []
    used = 0
    i = 0
    pattern = re.compile(r"%(?:(\d+)\$)?[-+ #0]*\d*(?:\.\d+)?[lh]*([difsuxX])")
    while i < len(text):
        if text.startswith("%%", i):
            out.append("%")
            i += 2
            continue
        m = pattern.match(text, i)
        if m:
            pos = int(m.group(1)) - 1 if m.group(1) else used
            if pos >= len(args):
                return None
            out.append("{ $" + args[pos] + " }")
            used += 1
            i = m.end()
            continue
        c = text[i]
        out.append({"{": '{ "{" }', "}": '{ "}" }'}.get(c, c))
        i += 1
    if used != len(args):
        print(f"  aviso: {ftl_id}: {used} argumentos en lugar de {len(args)}", file=sys.stderr)
        return None
    return "".join(out)


def ftl_value(text):
    """Valor de Fluent: una línea, o varias con sangría. Se quitan los espacios de los bordes."""
    lines = [l.strip() for l in text.strip("\n").split("\n")]
    lines = [l for l in lines if l] or [""]
    lines = [('{ "" }' + l) if l[:1] in "[*." else l for l in lines]
    if len(lines) == 1:
        return " " + lines[0]
    return "\n" + "\n".join("    " + l for l in lines)


def render(code, source, translations):
    out = [
        "# SPDX-License-Identifier: GPL-3.0-or-later",
        f"# Generado por tools/po2ftl.py desde {source} de Klavaro 3.14. No editar a mano.",
        "",
    ]
    n = 0
    for ftl_id, msgid, args in MESSAGES:
        text = translations(msgid)
        if not text:
            continue
        if re.search(r"_\w", msgid):
            # Quita el acelerador de GTK (`Te_clado:` → `Teclado:`).
            text = re.sub(r"_(?=\w)", "", text, count=1)
        value = printf_to_fluent(text, args, f"{code}:{ftl_id}")
        if value is None:
            continue
        out.append(f"{ftl_id} ={ftl_value(value)}")
        n += 1
    return "\n".join(out) + "\n", n


def main():
    po_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "upstream/klavaro-3.14/po"
    pot = parse_pot(po_dir / "klavaro.pot")
    missing = [m for _, m, _ in MESSAGES if m not in pot]
    if missing:
        for m in missing:
            print(f"no está en klavaro.pot: {m!r}", file=sys.stderr)
        sys.exit(1)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    codes = []
    text, n = render("C", "klavaro.pot", lambda m: m)
    (OUT_DIR / "C.ftl").write_text(text, encoding="utf-8")
    codes.append("C")
    print(f"C: {n}/{len(MESSAGES)}")
    for po in sorted(po_dir.glob("*.po")):
        code = po.stem
        entries = parse_po(po)
        text, n = render(code, f"po/{po.name}", entries.get)
        (OUT_DIR / f"{code}.ftl").write_text(text, encoding="utf-8")
        codes.append(code)
        print(f"{code}: {n}/{len(MESSAGES)}")

    rows = "\n".join(
        f'    ("{c}", include_str!("../locales/klavaro/{c}.ftl")),' for c in codes
    )
    CATALOG.write_text(
        "// SPDX-License-Identifier: GPL-3.0-or-later\n"
        "// Generado por tools/po2ftl.py. No editar a mano.\n"
        "//! Traducciones tomadas del Klavaro original, por código de idioma.\n\n"
        "/// `(código, contenido .ftl)` de cada idioma del original.\n"
        f"pub(crate) const ORIGINAL: &[(&str, &str)] = &[\n{rows}\n];\n",
        encoding="utf-8",
    )


def parse_pot(path):
    """Todos los msgid del .pot (aunque no tengan traducción)."""
    ids = set()
    field = None
    cur = ""
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        m = re.match(r'^(msgid|msgid_plural|msgstr(?:\[\d+\])?)\s+(".*")$', line)
        if m:
            if field == "msgid":
                ids.add(cur)
            field = m.group(1)
            cur = unquote(m.group(2))
        elif line.startswith('"') and field:
            cur += unquote(line)
    return ids


if __name__ == "__main__":
    main()
