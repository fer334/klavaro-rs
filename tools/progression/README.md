# Textos de la Progresión

Estas herramientas arman `assets/C.progression`, el catálogo con un texto por lección. Sólo usan la
biblioteca estándar de Python. La app no descarga nada: el catálogo va embebido en el binario.

## Regenerar el catálogo

Los tres pasos se pueden repetir sin perder trabajo.

```sh
python3 tools/progression/fetch.py      # baja los artículos a tools/progression/cache/ (ignorada por git)
python3 tools/progression/report.py     # cuenta los candidatos por fuente, tema y sección
python3 tools/progression/selection.py  # propone los textos y dice cuántos falta revisar
```

`fetch.py` retoma donde quedó, así que se puede cortar y volver a ejecutar. Sin caché, tarda unos 30
minutos.

## Revisar textos

`review.json` guarda los textos aprobados, fijos en su lección, y los artículos rechazados con su motivo.
Por eso un rechazo cambia sólo esa lección.

1. Mostrar lo que falta revisar:

   ```sh
   python3 tools/progression/selection.py --pending
   ```

2. Para descartar un artículo, agregar su identificador a `rejected` en `review.json`, con el motivo. Si
   ya estaba aprobado, quitar también su lección de `approved`.
3. Aprobar lo mostrado, menos lo rechazado. Los reemplazos nuevos quedan pendientes y se ven en la
   misma salida:

   ```sh
   python3 tools/progression/selection.py --approve --pending
   ```

4. Repetir hasta que diga `0 sin revisar`, y escribir el catálogo:

   ```sh
   python3 tools/progression/selection.py --write
   ```

Si cambian las reglas de `textrules.py`, los textos que ya no las cumplan dejan su lección libre y vuelven
a aparecer como pendientes.

## Cómo se elige cada texto

- **Largo:** el que dura unos 60 s a la meta de la lección, con ±15 % (`textrules.py`).
- **Forma:** oraciones completas desde el inicio del artículo, con el tema en la primera oración y sólo
  caracteres ASCII.
- **Dificultad:** un índice de 0 a 100 hecho con rareza y largo de palabras, largo de oraciones, mayúsculas
  y símbolos. Cada sección toma textos de una franja que sube con la meta.
- **Interés:** al menos 150 visitas diarias, sin lenguaje académico y con tope por tema en cada sección. A
  lo sumo un texto sobre religión o dioses por sección (`selection.py`).

## Sumar un idioma

1. **Licencia:** elegir wikis cuyos textos sean compatibles con GPLv3 y cargarlas en `sources.json`.
   `fetch.py` corta si la licencia declarada no es la esperada.
2. **Lista:** `sources.json` toma los títulos de la lista de artículos vitales de la Wikipedia en inglés.
   Para otro idioma hay que indicar una lista propia o mapear esos títulos.
3. **Reglas:** adaptar a ese idioma en `textrules.py` los caracteres permitidos (`ALLOWED`), las
   abreviaturas y los temas excluidos, y en `selection.py` la lista `RELIGIOUS`. La rareza de las palabras
   sale de los propios textos descargados.
4. **Catálogo:** apuntar `CATALOG` y `REVIEW` de `selection.py` al idioma nuevo y generar
   `assets/<idioma>.progression`.
5. **App:** hoy `LESSONS_LANGUAGE` (`crates/klavaro-app/src/sections.rs`) fija el inglés. Con más de un
   idioma, hay que elegirlo según los textos y ocultar la Progresión si no tiene catálogo. El progreso ya se
   guarda por idioma.
6. **Pruebas:** agregar el catálogo a `crates/klavaro-data/tests/assets.rs`.

## Pruebas

```sh
python3 -m unittest discover -s tools/progression
```
