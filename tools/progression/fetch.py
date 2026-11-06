#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Descarga los artículos candidatos para los textos de la Progresión.

Toma los títulos de la lista de artículos vitales de Wikipedia (sources.json) y baja la
introducción de cada uno en cada wiki, con su revisión y licencia, a una caché local ignorada
por git. Lo ya descargado no se vuelve a pedir, así que una descarga cortada se retoma
ejecutando el comando otra vez.

Uso:
    python3 tools/progression/fetch.py [--limit N] [--cache CARPETA]

Escribe, dentro de la caché:
    lists/<página>.json     lista de artículos vitales tal como se bajó
    licenses.json           licencia que declara cada wiki
    articles/<wiki>.jsonl   un artículo o un título sin artículo por línea
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Iterable

HERE = Path(__file__).resolve().parent
USER_AGENT = "klavaro-rs-text-fetch/0.1 (https://github.com/fer334/klavaro-rs)"
BATCH_SIZE = 20  # Máximo de la API para `extracts` con `exintro`.
PAUSE_SECONDS = 1.0
MAX_ATTEMPTS = 6
BACKOFF_SECONDS = 5.0
MAX_LAG_SECONDS = 5
TIMEOUT_SECONDS = 60


class FetchError(Exception):
    """No se pudo completar un pedido después de reintentarlo."""


class Response:
    """Respuesta de un pedido HTTP."""

    def __init__(self, status: int, headers: dict[str, str], body: bytes) -> None:
        self.status = status
        self.headers = headers
        self.body = body


def http_get(url: str) -> Response:
    """Hace un GET y devuelve también las respuestas de error, para poder reintentarlas."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as reply:
            return Response(reply.status, dict(reply.headers), reply.read())
    except urllib.error.HTTPError as error:
        return Response(error.code, dict(error.headers), error.read())


class Client:
    """Pedidos a Wikipedia con pausa entre ellos y reintentos."""

    def __init__(
        self,
        transport: Callable[[str], Response] = http_get,
        sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self.transport = transport
        self.sleep = sleep
        self.requests = 0

    def get(self, url: str) -> bytes:
        """Baja una URL. Reintenta ante errores de red, 429 y 5xx, y respeta `Retry-After`."""
        problem = ""
        for attempt in range(MAX_ATTEMPTS):
            if self.requests:
                self.sleep(PAUSE_SECONDS)
            self.requests += 1
            try:
                reply = self.transport(url)
            except OSError as error:
                problem = str(error)
                self.sleep(BACKOFF_SECONDS * 2**attempt)
                continue
            if reply.status == 200:
                return reply.body
            problem = f"HTTP {reply.status}"
            if reply.status != 429 and reply.status < 500:
                break
            self.sleep(wait_time(reply.headers, attempt))
        raise FetchError(f"{url}: {problem}")

    def api(self, wiki: str, params: dict[str, str]) -> dict:
        """Consulta la API de una wiki. Si los servidores están atrasados (`maxlag`), espera."""
        query = {"format": "json", "formatversion": "2", "maxlag": str(MAX_LAG_SECONDS), **params}
        url = f"https://{wiki}.wikipedia.org/w/api.php?{urllib.parse.urlencode(query)}"
        for attempt in range(MAX_ATTEMPTS):
            data = json.loads(self.get(url))
            error = data.get("error")
            if not error:
                return data
            if error.get("code") != "maxlag":
                raise FetchError(f"{wiki}: {error.get('code')}: {error.get('info')}")
            self.sleep(BACKOFF_SECONDS * 2**attempt)
        raise FetchError(f"{wiki}: los servidores siguen atrasados")

    def raw_page(self, wiki: str, title: str) -> bytes:
        """Baja el contenido de una página tal cual."""
        query = urllib.parse.urlencode({"title": title, "action": "raw"})
        return self.get(f"https://{wiki}.wikipedia.org/w/index.php?{query}")


def wait_time(headers: dict[str, str], attempt: int) -> float:
    """Espera antes de reintentar: la que pida el servidor o una que crece con cada intento."""
    for name, value in headers.items():
        if name.lower() == "retry-after":
            try:
                return max(float(value), BACKOFF_SECONDS)
            except ValueError:
                break
    return BACKOFF_SECONDS * 2**attempt


def write_atomic(path: Path, content: bytes) -> None:
    """Escribe en un temporal y lo renombra, para no dejar archivos a medias."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(content)
    os.replace(temporary, path)


def vital_articles(client: Client, sources: dict, cache: Path) -> dict[str, dict]:
    """Títulos de la lista de artículos vitales con su tema, en los niveles elegidos."""
    selected: dict[str, dict] = {}
    for name in sources["list_pages"]:
        path = cache / "lists" / f"{name}.json"
        if not path.exists():
            page = f"{sources['list_prefix']}{name}.json"
            write_atomic(path, client.raw_page(sources["list_wiki"], page))
        for title, info in json.loads(path.read_text(encoding="utf-8")).items():
            if info.get("level") in sources["levels"]:
                selected[title] = {"level": info["level"], "topic": info.get("topic", "")}
    return selected


def check_licenses(client: Client, sources: dict, cache: Path) -> dict[str, dict]:
    """Licencia de cada wiki. Corta si alguna no es la que permite usar sus textos."""
    path = cache / "licenses.json"
    known = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
    for wiki in sources["wikis"]:
        if wiki not in known:
            data = client.api(wiki, {"action": "query", "meta": "siteinfo", "siprop": "rightsinfo"})
            known[wiki] = data["query"]["rightsinfo"]
    write_atomic(path, json.dumps(known, ensure_ascii=False, indent=1).encode("utf-8"))
    expected = sources["license"].replace("CC BY-SA", "Attribution-Share Alike")
    for wiki, rights in known.items():
        if expected not in rights.get("text", ""):
            raise FetchError(f"{wiki}: licencia inesperada: {rights}")
    return known


def read_lines(path: Path) -> list[dict]:
    """Lee un `.jsonl`. Ignora una última línea cortada por una interrupción."""
    if not path.exists():
        return []
    lines = []
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            lines.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return lines


def requested_titles(lines: Iterable[dict]) -> set[str]:
    """Títulos que ya se pidieron, tuvieran artículo o no."""
    return {title for line in lines for title in line["requested"]}


def fetch_batch(client: Client, wiki: str, titles: list[str]) -> dict:
    """Pide un lote de títulos y junta las partes que la API entrega de a poco."""
    pages: dict[int, dict] = {}
    redirects: dict[str, str] = {}
    continuation: dict[str, str] = {}
    while True:
        data = client.api(
            wiki,
            {
                "action": "query",
                "titles": "|".join(titles),
                "redirects": "1",
                "prop": "extracts|info|pageprops",
                "exintro": "1",
                "explaintext": "1",
                "exlimit": str(BATCH_SIZE),
                "ppprop": "disambiguation",
                **continuation,
            },
        )
        query = data.get("query", {})
        for step in query.get("normalized", []) + query.get("redirects", []):
            redirects[step["from"]] = step["to"]
        for page in query.get("pages", []):
            known = pages.setdefault(page.get("pageid", id(page)), {})
            extract = known.get("extract")
            known.update(page)
            if extract and not page.get("extract"):
                known["extract"] = extract
        continuation = data.get("continue", {})
        if not continuation:
            return {"pages": pages, "redirects": redirects}


def batch_lines(wiki: str, titles: list[str], batch: dict, license_name: str, now: int) -> list[dict]:
    """Convierte un lote en líneas de la caché: cada título pedido queda en una línea."""
    by_title = {
        page["title"]: page for page in batch["pages"].values() if "missing" not in page
    }
    requested: dict[str, list[str]] = {}
    for title in titles:
        final = title
        seen = set()
        while final in batch["redirects"] and final not in seen:
            seen.add(final)
            final = batch["redirects"][final]
        requested.setdefault(final, []).append(title)
    lines = []
    for final, sources in requested.items():
        page = by_title.get(final)
        disambiguation = page and "disambiguation" in page.get("pageprops", {})
        if not page or disambiguation or not page.get("extract", "").strip():
            lines.append({"requested": sources, "article": None})
            continue
        lines.append(
            {
                "requested": sources,
                "article": {
                    "wiki": wiki,
                    "title": page["title"],
                    "pageid": page["pageid"],
                    "revision": page["lastrevid"],
                    "license": license_name,
                    "fetched_at": now,
                    "extract": page["extract"],
                },
            }
        )
    return lines


def fetch_wiki(
    client: Client, wiki: str, titles: list[str], cache: Path, license_name: str, limit: int | None
) -> int:
    """Baja los títulos que faltan de una wiki. Devuelve cuántos pidió."""
    path = cache / "articles" / f"{wiki}.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    done = requested_titles(read_lines(path))
    pending = [title for title in titles if title not in done]
    if limit is not None:
        pending = pending[:limit]
    # Si la última línea quedó cortada, la próxima escritura empieza en una línea nueva.
    if path.exists() and path.stat().st_size and not path.read_bytes().endswith(b"\n"):
        with path.open("a", encoding="utf-8") as out:
            out.write("\n")
    for start in range(0, len(pending), BATCH_SIZE):
        chunk = pending[start : start + BATCH_SIZE]
        batch = fetch_batch(client, wiki, chunk)
        lines = batch_lines(wiki, chunk, batch, license_name, int(time.time()))
        with path.open("a", encoding="utf-8") as out:
            for line in lines:
                out.write(json.dumps(line, ensure_ascii=False) + "\n")
        print(f"{wiki}: {start + len(chunk)}/{len(pending)}", flush=True)
    return len(pending)


def run(client: Client, sources: dict, cache: Path, limit: int | None = None) -> None:
    """Baja la lista, las licencias y los artículos."""
    selected = vital_articles(client, sources, cache)
    licenses = check_licenses(client, sources, cache)
    titles = sorted(selected)
    print(f"{len(titles)} títulos en la lista", flush=True)
    for wiki in sources["wikis"]:
        fetch_wiki(client, wiki, titles, cache, licenses[wiki]["text"], limit)
    write_atomic(
        cache / "selected.json", json.dumps(selected, ensure_ascii=False, indent=0).encode("utf-8")
    )
    print(f"{client.requests} pedidos", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--limit", type=int, help="máximo de títulos nuevos por wiki")
    parser.add_argument("--cache", type=Path, default=HERE / "cache")
    args = parser.parse_args()
    sources = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))
    try:
        run(Client(), sources, args.cache, args.limit)
    except FetchError as error:
        print(f"error: {error}. Lo descargado queda guardado: se puede volver a ejecutar.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
