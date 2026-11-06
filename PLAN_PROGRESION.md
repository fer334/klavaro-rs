# Plan: sección Progresión

Una parte nueva del programa, con forma de juego, organizada en secciones de velocidad: 20, 25…
75, 80… hasta 100 WPM. Cada sección tiene 30 lecciones y **cada lección pide un poco más de
velocidad que la anterior**, de modo que la lección 30 alcanza la meta que da nombre a la sección.
Cada lección terminada da un puntaje y de 1 a 5 estrellas, que quedan guardados.

El plan se divide en seis fases. Cada una deja algo completo, probado y que se puede mergear.

## Decisiones por confirmar

Estos valores están elegidos para poder avanzar. Todos salen de constantes, así que cambiarlos
más adelante es barato, salvo la 4 y la 5, que conviene definir antes de la Fase 1 y la Fase 2.

| # | Decisión | Valor propuesto | Por qué |
|---|---|---|---|
| 1 | Primera sección | 20 WPM | Fijaste el final (100), no el inicio. Da 17 secciones y 510 lecciones. |
| 2 | Estrellas para abrir la lección siguiente | 3 (80 % de la meta de la lección) | Permite seguir sin exigir la meta en cada texto. |
| 3 | Para aprobar una sección | Las 30 lecciones con 3 estrellas o más, y la lección 30 con 5 estrellas | La lección 30 pide la meta de la sección, así que aprobar demuestra haberla alcanzado. |
| 4 | Alcance del progreso | Por idioma, sin separar por teclado | Más simple. El teclado queda registrado en cada intento. |
| 5 | Idiomas del primer catálogo | Español (`es`) e inglés (`C`) | 510 textos revisados por idioma es bastante trabajo. |
| 6 | Precisión mínima para 5 estrellas | 95 % | Evita premiar velocidad con muchos errores. Agrega una condición a tu regla de la meta. |

## Reglas del juego

### Secciones y metas

La sección lleva el nombre de la meta a la que llega. Entre una sección y la siguiente hay 5 WPM,
repartidos en partes iguales entre las 30 lecciones:

```text
meta de la lección = meta de la sección − 5 + lección × 5 / 30
```

Cada lección sube 1/6 de WPM (unos 0,17). La meta se muestra con un decimal, y los cálculos usan
el valor exacto.

| Sección | Lección 1 | Lección 2 | Lección 15 | Lección 29 | Lección 30 |
|---|---|---|---|---|---|
| 20 WPM | 15,2 | 15,3 | 17,5 | 19,8 | 20,0 |
| 75 WPM | 70,2 | 70,3 | 72,5 | 74,8 | 75,0 |
| 80 WPM | 75,2 | 75,3 | 77,5 | 79,8 | 80,0 |
| 100 WPM | 95,2 | 95,3 | 97,5 | 99,8 | 100,0 |

La escalera es continua: la lección 1 de la sección 80 pide apenas más que la lección 30 de la
sección 75. Aprobar la lección 30 de la sección 100 termina la Progresión.

**Tamaño total:** 510 lecciones y 2550 estrellas posibles. A la meta y sin repetir, son unas
8,5 horas de escritura.

### Largo de los textos

Cada texto dura cerca de 60 s a la meta de su lección, con una tolerancia de ±15 % en el largo:
`caracteres ≈ meta × 5`.

| Sección | Caracteres, lección 1 | Caracteres, lección 30 |
|---|---|---|
| 20 WPM | 76 | 100 |
| 40 WPM | 176 | 200 |
| 60 WPM | 276 | 300 |
| 75 WPM | 351 | 375 |
| 100 WPM | 476 | 500 |

Los textos de EdClub rondan los 341 caracteres en todas sus metas de 65 a 85 WPM, así que su
duración baja a medida que sube la meta. Acá el largo acompaña a la meta para que las primeras
secciones no duren 3 minutos por lección.

### Tiempo máximo y estrellas

El tiempo máximo de una lección es `12 × caracteres / meta de la lección` segundos. Por ejemplo,
360 caracteres a 72 WPM dan 60 s.

Las lecciones usan el modo con corrección de Fluidez: solo terminan con el texto bien escrito. Por
eso la velocidad se calcula como `12 × caracteres / segundos`, y "terminar dentro del tiempo" y
"alcanzar la meta" son exactamente la misma condición. No se usan las WPM netas de `Stats`, que
suben un poco si se erra varias veces la misma letra.

Las estrellas salen de `r = velocidad / meta de la lección`:

| Estrellas | Velocidad |
|---|---|
| 5 | `r ≥ 1,00` (dentro del tiempo máximo) |
| 4 | `r ≥ 0,90` |
| 3 | `r ≥ 0,80` |
| 2 | `r ≥ 0,70` |
| 1 | Terminó la lección |

Tope por precisión: por debajo de 95 % el máximo es 4 estrellas, y por debajo de 90 % es 2.

### Puntaje, reintentos y desbloqueo

- **Puntaje:** el de `metrics::score`, que ya combina WPM, precisión y fluidez, para no mantener
  dos fórmulas. Se guarda en cada intento y se muestra junto con el récord.
- **Reintentos:** ilimitados. Pasarse del tiempo no corta la lección, solo da menos estrellas.
  Siempre vale el mejor resultado, así que nunca se pierden estrellas.
- **Desbloqueo:** lineal, según las decisiones 2 y 3. Se puede volver a jugar cualquier lección
  abierta.

### Dificultad

Lo que hace más difícil cada lección es **la meta de velocidad**. El texto no tiene que ser más
difícil en cada paso, lo que además facilita mucho armar el catálogo.

Los textos sí se vuelven algo más exigentes de una sección a otra. Cada uno recibe un índice `D` de
0 a 100, calculado al armar el catálogo con rasgos que no dependen del teclado: rareza de las
palabras en el corpus, largo de palabra, largo de oración, mayúsculas, puntuación y dígitos, y
letras acentuadas. Cada sección toma textos de una franja de `D` que sube con la meta.

No se usan métricas de dedos ni de filas porque el programa soporta 77 distribuciones, incluida
Dvorak. Tampoco sirven los `.words` como lista de frecuencias: no están ordenados por frecuencia.

## Diseño técnico

Respeta las reglas de [ARCHITECTURE.md](ARCHITECTURE.md): el core sin E/S, `klavaro-data` sobre
`&str`, solo la app conoce iced y los textos visibles viven en la capa de idioma.

| Crate | Cambio |
|---|---|
| `klavaro-core` | `progression.rs`: constantes, meta por lección, tiempo máximo, estrellas y desbloqueo. `Module::Progression`, que permite corrección y queda fuera de `Module::ALL`. |
| `klavaro-data` | `catalog.rs`: lee y escribe el catálogo, con errores tipados. |
| `klavaro-store` | Esquema 3: tabla de intentos y consultas de mejores resultados. |
| `klavaro-app` | `sections.rs` (estado sin iced, como `settings.rs`) y `gui/sections.rs` (vistas). `Screen::Sections` y `Screen::Section`. |
| `klavaro-i18n` | Claves nuevas en `app/C.ftl` y `app/es.ftl`. |
| `tools/progression/` | Scripts de Python, solo con biblioteca estándar, para obtener y seleccionar textos. |

`Module::Progression` queda fuera de `Module::ALL` porque esa lista alimenta las tarjetas 1–4 del
menú, las metas de Preferencias y la importación de archivos del Klavaro original. Los `match`
exhaustivos sobre `Module` obligan a decidir qué hace el módulo nuevo en cada lugar.

### Guardado

Una sola tabla de intentos. El mejor resultado y las estrellas por sección se derivan con SQL, así
no hay dos fuentes de verdad que puedan quedar desalineadas.

```sql
CREATE TABLE IF NOT EXISTS progression_attempts (
    id         INTEGER PRIMARY KEY,
    at         INTEGER NOT NULL,
    language   TEXT    NOT NULL,
    section    INTEGER NOT NULL,   -- meta de la sección: 20, 25… 100
    lesson     INTEGER NOT NULL,   -- 1 a 30
    target_wpm REAL    NOT NULL,   -- meta de la lección en ese intento
    text_id    TEXT    NOT NULL,
    keyboard   TEXT    NOT NULL,
    wpm        REAL    NOT NULL,
    accuracy   REAL    NOT NULL,
    fluidity   REAL    NOT NULL,
    score      REAL    NOT NULL,
    stars      INTEGER NOT NULL,
    elapsed_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS progression_by_lesson
    ON progression_attempts (language, section, lesson);
```

- El progreso se asocia a la posición (sección y lección), no al texto. Si una versión nueva cambia
  un texto, nadie pierde sus estrellas. `text_id` queda como registro.
- Cada intento también entra en `sessions`, en la misma transacción, para que los gráficos de
  progreso puedan usarlo.
- Las sesiones de Progresión no pasan por el mínimo de 500 caracteres ni entran al Top 10.
- El rastreo de errores y tiempos por tecla se alimenta como en los demás módulos.

### Catálogo

Un archivo por idioma en `assets/`, por ejemplo `assets/es.progression`, porque `build.rs` solo
embebe los archivos del primer nivel de esa carpeta. Formato de texto separado por tabuladores,
sin dependencias nuevas:

```text
# sección  lección  text_id  tema  D  fuente  título  revisión  texto
```

Con unos 300 caracteres de media por texto, más los metadatos, serían unos 200 KB por idioma, que
se suman a los 1,7 MB de datos actuales. Es asumible. Los textos van embebidos, de modo que la app
sigue funcionando sin internet y **no descarga nada mientras se usa**.

## Fuente de los textos

**Fuente recomendada: Wikipedia, en español e inglés.**

- **Licencia:** consulté la API de `es`, `en` y `simple` y las tres declaran CC BY-SA 4.0. Creative
  Commons declara GPLv3 compatible con BY-SA 4.0, y este proyecto es GPL-3.0-or-later. Exige
  atribución e indicar que el texto fue recortado. Conviene una revisión legal antes de publicar.
- **Descartadas:** los textos de EdClub, que no tenemos licencia para redistribuir, y Vikidia en
  español, que declara BY-SA 3.0 y no 4.0.

Medí el rendimiento con una muestra chica: 60 artículos al azar por idioma, tomando solo el inicio
de cada artículo, con largos equivalentes a 50–70 s y sin revisión de calidad.

| Meta | `es` | `en` | `simple` |
|---|---|---|---|
| 20 WPM | 8 % | 10 % | 13 % |
| 40 WPM | 17 % | 18 % | 25 % |
| 60 WPM | 8 % | 18 % | 28 % |
| 80 WPM | 5 % | 15 % | 22 % |
| 100 WPM | 5 % | 10 % | 20 % |

Esto indica dos cosas:

1. **El español es el cuello de botella**, sobre todo en las metas altas. Al 5 %, cubrir 30
   lecciones exige unos 600 artículos candidatos para esa sección.
2. **Al azar no alcanza.** Salen muchos artículos pobres, como especies poco conocidas o resultados
   electorales. Hay que partir de listas curadas, y la Fase 2 empieza por comprobar si existen y
   sirven: artículos destacados y buenos, y la lista de artículos que toda Wikipedia debería tener.

---

## Fase 1: el juego funcionando, con textos provisionales

**Resultado:** se puede jugar la Progresión de punta a punta, ganar estrellas, cerrar la app y
encontrar todo como estaba. Lo único provisional son los textos.

### Tareas

**Reglas (`klavaro-core`)**
- [x] Crear `progression.rs` con constantes, meta por lección, tiempo máximo y estrellas.
- [x] Agregar el estado de desbloqueo de lecciones y secciones a partir de los mejores resultados.
- [x] Agregar `Module::Progression` y resolver todos los `match` que pida el compilador.

**Guardado (`klavaro-store`)**
- [x] Pasar el esquema a la versión 3 con `progression_attempts`.
- [x] Implementar registrar intento y consultar mejores resultados por idioma y por sección.
- [x] Probar que una base de esquema 2 con datos se actualiza sin perder nada.

**Textos provisionales (`klavaro-app`)**
- [x] Generar el texto de cada lección cortando oraciones de los `.paragraphs` actuales, de forma
  determinista y con el largo que corresponde. Se permiten repeticiones. Se elimina en la Fase 4.

**Pantallas (`sections.rs` y `gui/sections.rs`)**
- [x] **Menú:** agregar una tarjeta destacada de Progresión con la tecla `N`, y actualizar el texto
  de atajos. Hay que distinguirla bien del botón Progreso, que ya existe y tiene un nombre parecido.
- [x] **Selección de sección:** las 17 secciones con su meta, estrellas `x/150`, lecciones aprobadas
  y estado (bloqueada, disponible o aprobada), y foco inicial en la sección actual.
- [x] **Sección:** grilla de 30 lecciones con la meta de cada una, estrellas, candado y la siguiente
  resaltada. Mostrar cuánto falta para aprobar.
- [x] **Lección:** reutilizar el `Tutor` y mostrar meta, tiempo máximo y tiempo transcurrido.
  Actualizar el tiempo con cada tecla, porque el ejecutor de iced de este proyecto no tiene
  temporizadores.
- [x] **Resultado:** estrellas, puntaje, WPM, precisión, tiempo contra el máximo, y si hay récord.
  Enter pasa a la siguiente si está abierta y Esc reintenta el mismo texto.
- [x] Hacer que `Ctrl++` y `Ctrl+-` no cambien de lección y que Volver regrese a la grilla de la sección.
- [x] Comprobar que `★` se ve en macOS.
- [ ] Comprobar que `★` se ve en Linux y Windows. Si no, dibujarla con `canvas`.
- [x] Agregar todos los textos a `C.ftl` y `es.ftl`.

### Criterios de aceptación

- [x] Las pruebas del core cubren la meta de las lecciones 1, 2 y 30 de varias secciones, cada límite
  de estrellas y de precisión, y el desbloqueo.
- [x] La meta sube en cada lección, también al pasar de una sección a la siguiente.
- [x] Una prueba confirma que 5 estrellas equivale a terminar dentro del tiempo máximo, también con errores corregidos.
- [x] Un resultado peor nunca baja las estrellas guardadas.
- [x] Un flujo completo sin ventana, con historial en memoria: jugar, guardar, recargar y encontrar
  el mismo estado.
- [x] Aprobar la sección de 20 WPM abre la de 25 WPM.
- [x] Toda la selección y el resultado se pueden usar solo con teclado.
- [x] Si falla el disco, se muestra el error y la app sigue funcionando, como en el resto del programa.
- [x] Pasan `cargo test`, `clippy` y `cargo deny`.

---

## Fase 2: descarga de textos públicos

**Resultado:** un comando reproducible baja artículos candidatos con su atribución a una caché
local. Puede hacerse en paralelo con la Fase 1.

### Tareas

- [ ] Evaluar las listas curadas por idioma y fijar las fuentes en un archivo versionado.
- [ ] Crear `tools/progression/fetch.py`, que use la API `action=query` con `prop=extracts` y
  `explaintext`, y guarde el id de revisión de cada artículo.
- [ ] Respetar las reglas de uso de la API: un `User-Agent` identificable, lotes, una pausa entre
  pedidos, y reintentos que respeten `maxlag` y `Retry-After`.
- [ ] Guardar la caché en `tools/progression/cache/`, ignorada por git, para no repetir descargas.
- [ ] Registrar por artículo: idioma, título, id de página, revisión, licencia, fecha y tema.
- [ ] Generar un informe de cuántos candidatos hay por idioma, tema y rango de largo.

### Criterios de aceptación

- [ ] Volver a ejecutar el comando sin cambios no hace pedidos nuevos.
- [ ] Una falla de red a mitad de camino se retoma sin perder lo ya descargado.
- [ ] Cada artículo guardado permite armar el enlace a su revisión exacta.
- [ ] **Control:** el informe alcanza la cantidad de candidatos que pide la Fase 3 en `es` y `C`. Si
  no, se amplían las fuentes antes de seguir.

---

## Fase 3: selección, dificultad y catálogo

**Resultado:** `assets/es.progression` y `assets/C.progression`, con 510 textos revisados cada uno.

### Tareas

**Limpieza y recorte (`select.py`)**
- [ ] Normalizar comillas, guiones y espacios, y quitar paréntesis de pronunciación y restos de
  plantillas.
- [ ] Cortar solo en oraciones completas, desde el inicio del artículo para conservar la
  presentación del tema, y terminar en punto.

**Filtros, tomados del análisis de EdClub**
- [ ] Oración más larga: hasta 34 palabras.
- [ ] Mayúsculas: hasta 7 %. Puntuación: hasta 5 %.
- [ ] Solo caracteres que se puedan escribir en el idioma, sin símbolos raros.
- [ ] El tema aparece en la primera oración.
- [ ] Rechazar textos que dependan de lo anterior ("Sin embargo…", "Esto…", "It…") y listas.
- [ ] Rechazar duplicados y casi duplicados.

**Dificultad y asignación**
- [ ] Calcular `D`, con la rareza de palabras medida sobre el propio corpus de candidatos.
- [ ] Definir la franja de `D` de cada sección, de modo que suba con la meta.
- [ ] Asignar a cada lección un candidato sin usar, dentro de su largo y de la franja de su sección.
- [ ] No repetir tema en dos lecciones seguidas, ni artículo en todo el catálogo.
- [ ] Que el proceso sea determinista e informe las lecciones sin cubrir.

**Revisión humana**
- [ ] Generar el doble de textos de los necesarios para poder descartar.
- [ ] Revisar sentido, tono y contenido sensible o desactualizado, y guardar las decisiones en un
  archivo versionado para que sobrevivan a una nueva generación.

### Criterios de aceptación

- [ ] 510 textos por idioma, sin artículos repetidos.
- [ ] El largo y la franja de `D` son correctos en cada lección.
- [ ] Todos los textos tienen fuente, revisión y licencia, y todos pasaron la revisión.
- [ ] Generar el catálogo dos veces produce archivos idénticos.

---

## Fase 4: catálogo real en la app

**Resultado:** la Progresión usa los textos definitivos, con atribución visible.

### Tareas

- [ ] Crear `klavaro-data/src/catalog.rs`, con lectura, escritura y errores tipados.
- [ ] Cargar el catálogo desde `assets.rs`, respetando `KLAVARO_ASSETS`.
- [ ] Eliminar los textos provisionales de la Fase 1.
- [ ] Mostrar el artículo de origen en el resultado, con enlace a la licencia.
- [ ] Agregar una pantalla de créditos e incluir el aviso de licencia en los paquetes de
  `tools/package.sh`. Agregar la atribución de los textos a `README.md`.
- [ ] Con un idioma sin catálogo, mostrar la Progresión deshabilitada con una explicación.
- [ ] Avisar cuando un texto tenga caracteres que el teclado actual no puede escribir, sin bloquear
  la lección.

### Criterios de aceptación

- [ ] Una prueba en `klavaro-data/tests/assets.rs` valida cada catálogo: 510 filas, todas las
  lecciones, largo y `D` correctos, y atribución completa.
- [ ] Los acentos y `ñ` se pueden escribir con teclas muertas en los catálogos `es`, incluida Dvorak.
- [ ] Un progreso guardado en la Fase 1 sigue siendo válido con el catálogo real.
- [ ] Los paquetes de macOS, Linux y Windows funcionan sin internet y sin carpeta de datos.

---

## Fase 5: pulido del juego

**Resultado:** se siente como un juego y el avance se ve de un vistazo.

### Tareas

- [ ] Mostrar el total de estrellas y el avance hacia los 100 WPM en la selección de sección.
- [ ] Destacar el récord y las estrellas nuevas en el resultado.
- [ ] Agregar una pantalla de sección aprobada y otra de Progresión completada.
- [ ] Reutilizar el pitido o un sonido del sistema al ganar estrellas, respetando las Preferencias.
- [ ] Agregar la Progresión a los gráficos de Progreso, con WPM, precisión y estrellas por sección.
- [ ] Mostrar las teclas más débiles de la lección y un acceso a la sesión especial de Adaptabilidad
  cuando corresponda.
- [ ] Agregar en Preferencias "Reiniciar progresión", con confirmación, para el idioma actual.

### Criterios de aceptación

- [ ] Reiniciar afecta solo al idioma elegido y no toca `sessions` ni el Top 10.
- [ ] Las vistas funcionan en tema claro y oscuro y con el tamaño mínimo de ventana.
- [ ] Las animaciones o sonidos no agregan demora antes de poder reintentar.

---

## Fase 6: calibración y lanzamiento

**Resultado:** las reglas están ajustadas con uso real y la sección está documentada y publicada.

### Tareas

- [ ] Jugar secciones de varias velocidades con distintas personas, si es posible.
- [ ] Revisar si la subida de meta se siente justa, si los umbrales de estrellas funcionan y si hay
  textos que tranquen. Ajustar constantes y reemplazar textos.
- [ ] Documentar la sección, el esquema 3 y el catálogo en `ARCHITECTURE.md`, y agregarla a `README.md`.
- [ ] Documentar cómo regenerar el catálogo y cómo sumar un idioma.
- [ ] Hacer la revisión final de licencia y atribución.
- [ ] Generar y probar los paquetes de las tres plataformas, y publicar la versión.

### Criterios de aceptación

- [ ] Se puede completar un recorrido de las 17 secciones sin bloqueos, con un script o datos de prueba.
- [ ] Sin textos señalados como defectuosos pendientes.
- [ ] La documentación alcanza para sumar un idioma nuevo sin leer el código de la herramienta.

---

## Orden y dependencias

```text
Fase 1 (juego) ──────────────┐
                             ├─► Fase 4 (integración) ─► Fase 5 (pulido) ─► Fase 6 (lanzamiento)
Fase 2 (descarga) ─► Fase 3 ─┘
```

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Pocos textos buenos en español para las metas altas | Control al final de la Fase 2. Unir hasta dos secciones de un mismo artículo, sumar más listas curadas o escribir textos propios. |
| 1020 textos para revisar por idioma | Aplicar filtros estrictos antes, revisar por lotes y conservar las decisiones. Es el mayor costo del plan. |
| Metas difíciles de alcanzar y abandono | El umbral de 3 estrellas permite seguir avanzando. Se ajusta en la Fase 6. |
| Confusión entre "Progreso" y "Progresión" | Separarlas visualmente en el menú y revisar el nombre al probar la Fase 1. |
| Errores de atribución o licencia | Fuente y revisión obligatorias, validadas por una prueba, y revisión antes de publicar. |
