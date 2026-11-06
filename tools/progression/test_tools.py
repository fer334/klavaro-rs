# SPDX-License-Identifier: GPL-3.0-or-later
"""Pruebas de las herramientas de textos. Uso: python3 -m unittest discover tools/progression"""

from __future__ import annotations

import collections
import json
import tempfile
import unittest
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import fetch
import report
import selection
import textrules


def reply(data: object, status: int = 200, headers: dict | None = None) -> fetch.Response:
    return fetch.Response(status, headers or {}, json.dumps(data).encode())


SOURCES = {
    "wikis": ["simple"],
    "license": "CC BY-SA 4.0",
    "list_wiki": "en",
    "list_prefix": "Wikipedia:Vital articles/data/",
    "list_pages": ["A"],
    "levels": [1, 2],
}
VITAL = {
    "Alpha": {"level": 1, "topic": "Science"},
    "Beta": {"level": 2, "topic": "Arts"},
    "Gamma": {"level": 5, "topic": "Arts"},
    "Ghost": {"level": 2, "topic": "Arts"},
    "Moved": {"level": 2, "topic": "Arts"},
    "Many": {"level": 2, "topic": "Arts"},
}


class FakeWiki:
    """Responde como la API para una lista fija de páginas."""

    def __init__(self) -> None:
        self.urls: list[str] = []
        self.fail_after: int | None = None
        self.license = "Creative Commons Attribution-Share Alike 4.0"

    def page(self, title: str) -> dict:
        if title == "Ghost":
            return {"title": title, "missing": True}
        extra = {"pageprops": {"disambiguation": ""}} if title == "Many" else {}
        return {
            "title": title,
            "pageid": sum(map(ord, title)),
            "lastrevid": 7,
            "extract": f"{title} text.",
            **extra,
        }

    def __call__(self, url: str) -> fetch.Response:
        if self.fail_after is not None and len(self.urls) >= self.fail_after:
            raise OSError("sin red")
        self.urls.append(url)
        parts = urlparse(url)
        query = parse_qs(parts.query)
        if query.get("action") == ["raw"]:
            return reply(VITAL)
        if query.get("prop") == ["pageviews"]:
            pages = [
                {"title": t, "pageviews": {"d1": 10, "d2": None, "d3": 30}}
                for t in query["titles"][0].split("|")
                if t != "Ghost"
            ]
            return reply({"query": {"pages": pages}})
        if query.get("meta") == ["siteinfo"]:
            return reply({"query": {"rightsinfo": {"text": self.license}}})
        titles = query["titles"][0].split("|")
        redirects = [{"from": "Moved", "to": "Beta"}] if "Moved" in titles else []
        final = {("Beta" if t == "Moved" else t) for t in titles}
        pages = [self.page(t) for t in sorted(final)]
        return reply({"query": {"redirects": redirects, "pages": pages}})


class ClientTests(unittest.TestCase):
    def client(self, replies: list) -> tuple[fetch.Client, list[float]]:
        waits: list[float] = []
        answers = iter(replies)

        def transport(_url: str) -> fetch.Response:
            answer = next(answers)
            if isinstance(answer, Exception):
                raise answer
            return answer

        return fetch.Client(transport, waits.append), waits

    def test_identifies_itself_and_asks_for_maxlag(self) -> None:
        self.assertIn("github.com", fetch.USER_AGENT)
        seen = []

        def transport(url: str) -> fetch.Response:
            seen.append(url)
            return reply({"query": {}})

        fetch.Client(transport, lambda _s: None).api("en", {"action": "query"})
        self.assertIn("maxlag=5", seen[0])

    def test_pauses_between_requests(self) -> None:
        client, waits = self.client([reply({}), reply({})])
        client.get("u")
        client.get("u")
        self.assertEqual(waits, [fetch.PAUSE_SECONDS])

    def test_respects_retry_after(self) -> None:
        busy = reply({}, 429, {"Retry-After": "30"})
        client, waits = self.client([busy, reply({"ok": 1})])
        self.assertEqual(json.loads(client.get("u")), {"ok": 1})
        self.assertIn(30.0, waits)

    def test_retries_network_errors_and_server_errors(self) -> None:
        client, _ = self.client([OSError("x"), reply({}, 503), reply({"ok": 1})])
        self.assertEqual(json.loads(client.get("u")), {"ok": 1})

    def test_waits_when_servers_lag(self) -> None:
        lag = reply({"error": {"code": "maxlag", "info": "lag"}})
        client, waits = self.client([lag, reply({"query": {}})])
        self.assertEqual(client.api("en", {}), {"query": {}})
        self.assertIn(fetch.BACKOFF_SECONDS, waits)

    def test_gives_up_on_client_errors_and_repeated_failures(self) -> None:
        client, _ = self.client([reply({}, 404)])
        with self.assertRaises(fetch.FetchError):
            client.get("u")
        client, _ = self.client([OSError("x")] * fetch.MAX_ATTEMPTS)
        with self.assertRaises(fetch.FetchError):
            client.get("u")

    def test_other_api_errors_are_not_retried(self) -> None:
        client, _ = self.client([reply({"error": {"code": "bad", "info": "no"}})])
        with self.assertRaises(fetch.FetchError):
            client.api("en", {})


class RunTests(unittest.TestCase):
    def setUp(self) -> None:
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.cache = Path(folder.name)
        self.wiki = FakeWiki()

    def run_fetch(self, limit: int | None = None) -> fetch.Client:
        client = fetch.Client(self.wiki, lambda _s: None)
        fetch.run(client, SOURCES, self.cache, limit)
        return client

    def lines(self) -> list[dict]:
        return fetch.read_lines(self.cache / "articles" / "simple.jsonl")

    def test_records_attribution_and_every_requested_title(self) -> None:
        self.run_fetch()
        lines = self.lines()
        self.assertEqual(
            fetch.requested_titles(lines), {"Alpha", "Beta", "Ghost", "Moved", "Many"}
        )
        beta = next(l for l in lines if "Beta" in l["requested"])
        self.assertEqual(sorted(beta["requested"]), ["Beta", "Moved"])
        article = beta["article"]
        self.assertEqual(article["revision"], 7)
        self.assertEqual(article["wiki"], "simple")
        self.assertIn("Share Alike 4.0", article["license"])
        self.assertTrue(article["pageid"])

    def test_records_page_views(self) -> None:
        self.run_fetch()
        views = json.loads((self.cache / "views.json").read_text(encoding="utf-8"))
        self.assertEqual(views["Alpha"], 30)
        self.assertEqual(views["Ghost"], 0, "sin datos cuenta como cero")
        self.assertNotIn("Gamma", views, "fuera de los niveles elegidos")

    def test_skips_missing_pages_and_disambiguations(self) -> None:
        self.run_fetch()
        for title in ("Ghost", "Many"):
            line = next(l for l in self.lines() if title in l["requested"])
            self.assertIsNone(line["article"], title)

    def test_second_run_makes_no_requests(self) -> None:
        self.run_fetch()
        before = len(self.wiki.urls)
        self.assertEqual(self.run_fetch().requests, 0)
        self.assertEqual(len(self.wiki.urls), before)

    def test_resumes_after_a_network_failure(self) -> None:
        self.run_fetch(limit=2)
        self.assertEqual(len(fetch.requested_titles(self.lines())), 2)
        self.wiki.fail_after = len(self.wiki.urls)
        with self.assertRaises(fetch.FetchError):
            client = fetch.Client(self.wiki, lambda _s: None)
            fetch.run(client, SOURCES, self.cache)
        self.assertEqual(len(fetch.requested_titles(self.lines())), 2)
        self.wiki.fail_after = None
        self.run_fetch()
        self.assertEqual(len(fetch.requested_titles(self.lines())), 5)

    def test_tolerates_a_cut_last_line(self) -> None:
        self.run_fetch(limit=2)
        path = self.cache / "articles" / "simple.jsonl"
        path.write_text(path.read_text(encoding="utf-8") + '{"requested": ["Be', encoding="utf-8")
        self.run_fetch()
        self.assertEqual(len(fetch.requested_titles(self.lines())), 5)

    def test_stops_on_an_unexpected_license(self) -> None:
        self.wiki.license = "All rights reserved"
        with self.assertRaises(fetch.FetchError):
            self.run_fetch()
        self.assertFalse((self.cache / "articles").exists())

    def test_joins_extracts_delivered_in_parts(self) -> None:
        answers = iter(
            [
                reply(
                    {
                        "continue": {"excontinue": 1, "continue": "||"},
                        "query": {
                            "pages": [
                                {"pageid": 1, "title": "A", "lastrevid": 3, "extract": "One."},
                                {"pageid": 2, "title": "B", "lastrevid": 4},
                            ]
                        },
                    }
                ),
                reply(
                    {
                        "query": {
                            "pages": [
                                {"pageid": 1, "title": "A", "lastrevid": 3},
                                {"pageid": 2, "title": "B", "lastrevid": 4, "extract": "Two."},
                            ]
                        }
                    }
                ),
            ]
        )
        client = fetch.Client(lambda _u: next(answers), lambda _s: None)
        batch = fetch.fetch_batch(client, "en", ["A", "B"])
        self.assertEqual(batch["pages"][1]["extract"], "One.")
        self.assertEqual(batch["pages"][2]["extract"], "Two.")


class RuleTests(unittest.TestCase):
    def test_section_lengths_follow_the_goal(self) -> None:
        low, high = textrules.section_chars(100)
        self.assertAlmostEqual(low, (95 + 1 / 6) * 5 * 0.85)
        self.assertAlmostEqual(high, 575)
        self.assertEqual(len(textrules.sections()), 17)

    def test_clean_removes_pronunciation_and_typography(self) -> None:
        text = textrules.clean("Aristotle (Greek: Ἀριστοτέλης; 384 BC) was “wise”.\nHe wrote.")
        self.assertEqual(text, 'Aristotle was "wise". He wrote.')
        self.assertEqual(textrules.clean("Tea (a drink) is good."), "Tea (a drink) is good.")
        self.assertEqual(textrules.clean("The papaya (, US: ) is a plant."), "The papaya is a plant.")
        self.assertEqual(textrules.clean("Ada () (1815) wrote."), "Ada (1815) wrote.")

    def test_sentences_ignore_abbreviations_and_initials(self) -> None:
        text = "Dr. Smith lives in the U.S. Army base. J. Doe met St. John. It ended."
        self.assertEqual(
            textrules.split_sentences(text),
            ["Dr. Smith lives in the U.S. Army base.", "J. Doe met St. John.", "It ended."],
        )

    def test_filters(self) -> None:
        good = "Frogs live on land and in fresh water. They lay eggs."
        self.assertTrue(textrules.is_valid(good, "Frogs"))
        self.assertFalse(textrules.is_valid(good[:-1], "Frogs"), "sin punto final")
        self.assertFalse(textrules.is_valid("Frogs cost 5 % of é.", "Frogs"), "caracteres raros")
        self.assertFalse(textrules.is_valid("Toads are fine. Frogs too.", "Frogs"), "sin tema")
        self.assertFalse(textrules.is_valid("Frogs IN NASA AND UNESCO.", "Frogs"), "mayúsculas")
        self.assertFalse(textrules.is_valid("Frogs, a, b, c, d, e, f.", "Frogs"), "puntuación")
        self.assertFalse(textrules.is_valid("Frogs " + "x " * 40 + "end.", "Frogs"), "muy larga")
        self.assertFalse(textrules.is_valid("Frogs may refer to these.", "Frogs"))

    def test_filters_technical_topics(self) -> None:
        self.assertFalse(textrules.is_valid("In mathematics, frogs are a set.", "Frogs"))
        self.assertFalse(textrules.is_valid("Frogs is a subfield of biology here.", "Frogs"))
        self.assertTrue(textrules.is_valid("Frogs live near a quiet pond.", "Frogs"))

    def test_filters_unsuitable_content(self) -> None:
        self.assertFalse(textrules.is_valid("Frogs were killed in the old town.", "Frogs"))
        self.assertFalse(textrules.is_valid("Frogs are currently in the old town.", "Frogs"))
        self.assertFalse(textrules.is_valid("Frogs live (, in) the old town.", "Frogs"))
        self.assertFalse(textrules.is_valid("Frogs are 12345678 of 12345678 old.", "Frogs"), "dígitos")
        self.assertFalse(textrules.is_valid("Frogs (a) (b) (c) live in town.", "Frogs"), "paréntesis")
        self.assertFalse(textrules.is_valid('Frogs say "ribbit in the town.', "Frogs"), "comillas")
        self.assertFalse(textrules.is_valid("Frogs ( FROG-z) live in town.", "Frogs"), "pronunciación")
        self.assertFalse(textrules.is_valid("For the band, see frogs. Frogs live here.", "Frogs"))

    def test_topic_ignores_articles_and_qualifiers(self) -> None:
        self.assertEqual(textrules.topic_word("The Beatles"), "beatles")
        self.assertEqual(textrules.topic_word("Mercury (planet)"), "mercury")

    def test_cuts_and_sections(self) -> None:
        sentence = "Frogs live on land and in fresh water near the old and quiet river bank. "
        cuts = textrules.valid_cuts(sentence * 12, "Frogs")
        self.assertEqual(len(cuts[0]), len(sentence) - 1)
        self.assertLessEqual(len(cuts[-1]), 575)
        fits = textrules.sections_for(cuts)
        self.assertIn(20, fits)
        self.assertIn(100, fits)


class SelectionTests(unittest.TestCase):
    TOPICS = ["Arts", "Geography", "Technology", "Everyday life", "People"]

    @classmethod
    def setUpClass(cls) -> None:
        cls.cuts = []
        number = 0
        for section in textrules.sections():
            for lesson in range(1, textrules.LESSONS + 1):
                for option in range(7):
                    number += 1
                    size = round(selection.target_chars(section, lesson))
                    letters = str.maketrans("0123456789", "abcdefghij")
                    words = " ".join(f"{number}z{i}".translate(letters) for i in range(size))
                    text = words[: size - 1].rsplit(" ", 1)[0] + "."
                    cls.cuts.append(
                        {
                            "id": f"simple-{number}",
                            "wiki": "simple",
                            "title": f"T{number}",
                            "revision": number,
                            "seed": f"t{number}",
                            "topic": cls.TOPICS[number % len(cls.TOPICS)],
                            "level": 1 + option % 4,
                            "text": text,
                            "d": selection.section_center(section),
                        }
                    )

    def test_covers_every_lesson_without_repeating_articles_or_topics_in_a_row(self) -> None:
        rows, missing = selection.assign(self.cuts, set())
        self.assertEqual(missing, [])
        self.assertEqual(len(rows), 510)
        self.assertEqual(len({r["seed"] for r in rows}), 510)
        for first, second in zip(rows, rows[1:]):
            if first["section"] == second["section"]:
                self.assertNotEqual(first["topic"], second["topic"])

    def test_lengths_and_difficulty_follow_the_section(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        for row in rows:
            target = selection.target_chars(row["section"], row["lesson"])
            self.assertLessEqual(abs(len(row["text"]) - target), target * textrules.LENGTH_TOLERANCE)
            self.assertLessEqual(abs(row["d"] - selection.section_center(row["section"])), selection.D_WIDTH)
        centers = [selection.section_center(s) for s in textrules.sections()]
        self.assertEqual(centers, sorted(centers))

    def test_is_deterministic_and_a_rejection_makes_room_for_another_text(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        again, _ = selection.assign(self.cuts, set())
        self.assertEqual(selection.catalog_text(rows), selection.catalog_text(again))
        rejected = rows[0]["id"]
        replaced, missing = selection.assign(self.cuts, {rejected})
        self.assertEqual(missing, [])
        self.assertNotIn(rejected, {r["id"] for r in replaced})

    def test_approved_texts_stay_and_only_the_rejected_lesson_changes(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        rejected = rows[100]["id"]
        review = selection.approve(rows, {"approved": {}, "rejected": {rejected: "error"}})
        self.assertEqual(len(review["approved"]), 509)
        self.assertEqual(list(review["approved"])[:2], ["20.1", "20.2"])
        new, missing = selection.assign(self.cuts, {rejected}, review["approved"])
        self.assertEqual(missing, [])
        pending = selection.pending_rows(new, review)
        self.assertEqual([selection.slot_of(r) for r in pending], [selection.slot_of(rows[100])])
        self.assertNotEqual(pending[0]["id"], rejected)
        for first, second in zip(new, new[1:]):
            if first["section"] == second["section"]:
                self.assertNotEqual(first["topic"], second["topic"])
        self.assertEqual(len({r["seed"] for r in new}), 510)

    def test_approving_uses_what_was_shown_in_the_last_proposal(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "proposal.tsv"
            path.write_text(selection.catalog_text(rows[:3]), encoding="utf-8")
            shown = selection.read_proposal(path)
        review = selection.approve(shown, {"approved": {}, "rejected": {rows[1]["id"]: "error"}})
        self.assertEqual(list(review["approved"]), ["20.1", "20.3"])
        self.assertEqual(review["approved"]["20.1"]["fingerprint"], selection.fingerprint(rows[0]))

    def test_the_same_title_is_not_used_twice_across_wikis(self) -> None:
        twin = [dict(c) for c in self.cuts]
        for cut in twin:
            cut["title"] = "Same"
        rows, _ = selection.assign(twin, set())
        self.assertEqual(len(rows), 1)

    def test_prefers_popular_topics_and_limits_academic_ones(self) -> None:
        cuts = [
            {**c, "topic": "Mathematics", "views": 100_000} if i % 2 else {**c, "views": 200}
            for i, c in enumerate(self.cuts)
        ]
        rows, _ = selection.assign(cuts, set())
        by_section = collections.Counter(
            r["section"] for r in rows if r["topic"] == "Mathematics"
        )
        self.assertLessEqual(max(by_section.values()), 1)
        unpopular = [{**c, "views": 10} for c in self.cuts]
        self.assertEqual(selection.assign(unpopular, set())[0], [])

    def test_allows_one_religious_text_per_section_at_most(self) -> None:
        cuts = [{**c, "title": "Holy"} if i % 2 else c for i, c in enumerate(self.cuts)]
        rows, _ = selection.assign(cuts, set())
        by_section = collections.Counter(r["section"] for r in rows if selection.is_religious(r))
        self.assertLessEqual(max(by_section.values()), 1)

    def test_detects_religious_texts_by_title_or_first_sentence(self) -> None:
        self.assertTrue(selection.is_religious({"title": "Bible", "text": "A book. It is old."}))
        self.assertTrue(selection.is_religious({"title": "Zeus", "text": "Zeus is a Greek god."}))
        self.assertFalse(selection.is_religious({"title": "Milk", "text": "Milk is food."}))
        self.assertFalse(selection.is_religious({"title": "Christopher Columbus", "text": "He sailed."}))
        self.assertFalse(selection.is_religious({"title": "Jewellery", "text": "Rings are nice."}))
        self.assertFalse(
            selection.is_religious({"title": "Milk", "text": "Milk is food. A god drank it."})
        )

    def test_a_changed_text_needs_a_new_review(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        row = rows[0]
        review = selection.approve([row], {"approved": {}, "rejected": {}})
        self.assertEqual(selection.pending_rows([row], review), [])
        changed = {**row, "text": row["text"] + " More."}
        self.assertEqual(selection.pending_rows([changed], review), [changed])

    def test_uncovered_lessons_are_reported(self) -> None:
        rows, missing = selection.assign([], set())
        self.assertEqual(rows, [])
        self.assertEqual(len(missing), 510)

    def test_difficulty_grows_with_rare_words_and_long_sentences(self) -> None:
        frequency = selection.word_rarity(["the cat sat", "the dog ran", "the sun set", "zygote"])
        easy = selection.features("The cat sat. The dog ran.", frequency)
        hard = selection.features("Zygote, the zygote zygote zygote zygote zygote zygote.", frequency)
        difficulty = selection.Difficulty([easy, hard])
        self.assertLess(difficulty(easy), difficulty(hard))

    def test_catalog_lines_have_all_the_attribution(self) -> None:
        rows, _ = selection.assign(self.cuts, set())
        fields = selection.catalog_text(rows[:1]).splitlines()[-1].split("\t")
        self.assertEqual(len(fields), 9)
        self.assertEqual(fields[0:2], ["20", "1"])
        self.assertEqual(fields[5], "simple")


class ReportTests(unittest.TestCase):
    def test_one_article_counts_once_across_sections(self) -> None:
        both = {20, 25}
        candidates = [
            {"seed": "A", "fits": both},
            {"seed": "A", "fits": both},
            {"seed": "B", "fits": {25}},
        ]
        given = report.allocate(candidates, 1)
        self.assertEqual(given[20], 1)
        self.assertEqual(given[25], 1)
        self.assertEqual(report.allocate(candidates[:2], 1)[20] + report.allocate(candidates[:2], 1)[25], 1)

    def test_report_tells_if_it_is_enough(self) -> None:
        with tempfile.TemporaryDirectory() as folder:
            cache = Path(folder)
            (cache / "articles").mkdir()
            (cache / "selected.json").write_text(json.dumps({"Frogs": {"level": 1, "topic": "Life"}}))
            extract = "Frogs live on land and in fresh water near the old river bank. " * 2
            line = {
                "requested": ["Frogs"],
                "article": {"wiki": "simple", "title": "Frogs", "extract": extract},
            }
            (cache / "articles" / "simple.jsonl").write_text(json.dumps(line) + "\n")
            text, enough = report.report(cache, ["simple"], 60)
            self.assertFalse(enough)
            self.assertIn("NO alcanza", text)
            self.assertIn("Life", text)


if __name__ == "__main__":
    unittest.main()
