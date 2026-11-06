# SPDX-License-Identifier: GPL-3.0-or-later
"""Pruebas de las herramientas de textos. Uso: python3 -m unittest discover tools/progression"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import fetch
import report
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
