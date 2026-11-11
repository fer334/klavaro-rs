# Plan: ver y practicar las transiciones lentas

Dos cosas nuevas para la **Progresión**:

1. **Ver** qué pares de letras (`er`, `th`, `io`) escribe más lento o con más errores el
   estudiante.
2. **Practicarlos** en un ejercicio corto, de alrededor de un minuto, como una lección. Esta
   parte es **condicional**: se decide en la Fase 4, con datos reales.

Es el equivalente de «Practicar teclas débiles», pero la unidad es el par, no la tecla.

La regla de fondo:

```text
par débil = suficientes muestras ∧ (lento comparado con el propio estudiante ∨ se falla seguido)
```

Alcance: sólo la Progresión, que es el único módulo que mide pulsaciones y liberaciones
([PLAN_TRANSICIONES.md](PLAN_TRANSICIONES.md)). Usa su idioma y sus textos (`C`). No cambia
puntaje, estrellas ni el avance de las lecciones.

## Qué dice la evidencia

Seis trabajos, leídos en noviembre de 2026. De Salthouse y de DeFulio sólo se consiguió el
resumen; de Pinet, todo menos la discusión final.

| Trabajo | Qué encontró | Qué cambia en este plan |
|---|---|---|
| Dhakal y otros (2018), 168 000 personas | IKI promedio 239 ms. Los rápidos promedian ~120 ms (DE 11 ms); los lentos, más de 480 ms. Piso de ~60 ms | Los «400 ms → 100 ms» son la diferencia entre **grupos** de personas, no el resultado de un entrenamiento. La meta es relativa a cada estudiante (decisión 7). |
| Dhakal y otros (2018) | El IKI de los pares de una mano o de manos alternadas se correlaciona −0,7 con la velocidad; el de las letras repetidas, sólo −0,32 | Las letras repetidas (`ll`, `ee`) quedan afuera (decisión 2). |
| Dhakal y otros (2018) | La letra que sigue a un espacio es más lenta (inicio de palabra). Quien deja y corrige menos errores es más rápido | Los pares con espacio quedan afuera (decisión 2). Los errores cuentan (decisión 8). |
| Dhakal y otros (2018) | El rollover se correlaciona 0,73 con la velocidad; los rápidos lo usan en 40–70 % de las teclas, con ~30 ms de solapamiento | El rollover sigue siendo un dato, no una meta: es correlación. |
| Feit y otros (2016), 30 personas con captura de movimiento | IKI por tipo: manos alternadas 141–151 ms, misma mano y otro dedo ~161 ms, mismo dedo 195–203 ms | Confirma los factores que ya usa el código (0,9 / 1,0 / 1,25): 150/168 ≈ 0,9 y 199/161 ≈ 1,24 (decisión 6). |
| Feit y otros (2016) | Predicen la velocidad: usar siempre el mismo dedo para cada tecla, preparar la tecla siguiente y mover poco la mano. No hace falta mecanografía formal | Nada de esto se mide con el teclado. Refuerza practicar con texto que deja ver y preparar lo que viene (decisión 11). |
| DeFulio y otros (2011) | Los principiantes llegaron a escribir con soltura en casi la mitad del tiempo practicando con palabras en lugar de letras mezcladas | Todo el texto son palabras reales; nunca el par suelto ni sílabas inventadas (decisión 11). |
| Pinet y otros (2022), 1301 estudiantes | Los pares más frecuentes se escriben más rápido, y más todavía en los mejores. La cantidad de uso diario distingue a los mejores; la práctica deliberada que declararon, no | La práctica es corta y optativa, y no traba el avance. Se ordena por frecuencia del par (decisión 9). |
| Ruopp y otros (2026), preprint sin revisión | IKI por frecuencia del par: baja 200 ms, media 168 ms, alta 141 ms. Con 10 repeticiones en una sesión, el par poco frecuente baja ~5 ms | En un minuto la mejora es de pocos milisegundos: el resultado no promete un «antes y después» (decisión 13). La mejora se mira a lo largo de los días (decisión 14). |
| Ruopp y otros (2026) | La frecuencia de la **palabra** acelera además de la del par. La rapidez no se correlacionó con su medida de habilidad, que incluye errores | Palabras comunes antes que raras (decisión 11). No se premia velocidad con errores. |
| Salthouse (1986), 20 horas de práctica | Mejoran más las teclas en las que los movimientos de dos dedos se pueden solapar | Los pares de dedos distintos tienen más margen que los del mismo dedo: otra razón para ajustar por tipo (decisión 6). |

Lo que la evidencia **no** dice: ninguno de los seis probó que entrenar pares lentos mejore la
velocidad general. Lo que hay son correlaciones fuertes (pares rápidos ↔ escritura rápida) y un
resultado causal sobre el material (palabras mejor que letras mezcladas). Por eso primero se
construye la vista, que no depende de ninguna hipótesis, y la práctica queda para después de
mirar los datos (Fase 4).

## Decisiones por confirmar

| # | Decisión | Valor propuesto | Por qué |
|---|---|---|---|
| 1 | Duración de la práctica | Igual que una lección de Progresión: `meta_wpm × 5` caracteres, con la meta de la sección en curso (`Ladder::current_section`). A 40 PPM son 200 caracteres | Pedido. Sale de `SECONDS_PER_LESSON`, la misma constante de las lecciones. |
| 2 | Qué es un par | Dos letras minúsculas **distintas** seguidas dentro de una palabra. Sin espacios, signos, números, mayúsculas ni letras repetidas | Dhakal: las repetidas casi no predicen la velocidad y el inicio de palabra es lento por otra razón. Además el generador busca en una lista de palabras. |
| 3 | Muestras guardadas | Los últimos 30 IKI por par, como `MAX_TT_SAVED` en las teclas | Con una ventana corta, un par que mejora deja de ser débil enseguida. |
| 4 | Qué muestra cuenta | Sólo transiciones con las dos letras correctas a la primera y con tiempos medidos. IKI mayor a 1 s se descarta | Un IKI de 2 s es una distracción. Es la misma condición de «medida» de `transitions::analyze`. |
| 5 | Mínimo de muestras | 6 por par | Menos que eso es ruido. |
| 6 | Tiempo de un par | Mediana de sus muestras, dividida por el factor del tipo (otra mano 0,9; misma mano 1,0; mismo dedo 1,25) | Sin el factor, los pares del mismo dedo salen siempre como los más lentos. Feit da casi los mismos cocientes. |
| 7 | Par lento | Tiempo ajustado ≥ 1,25 × la mediana de los tiempos ajustados de todos los pares del estudiante. Hacen falta 8 pares medidos para tener esa referencia | Se compara al estudiante consigo mismo, no contra 100 ms. Se ajusta solo a medida que mejora. |
| 8 | Par que se falla | Errores en ≥ 15 % de las veces que se escribió, con el mínimo de muestras y al menos 2 errores | Un par rápido que falla seguido también es débil. |
| 9 | Orden | Por `gravedad × (1 + frecuencia del par en la lista de palabras)` | Un `er` lento cuesta más que un `zx` lento. |
| 10 | Cuántos pares por práctica | Hasta 4. Con menos de 2 pares débiles, el botón no aparece | Con más pares, cada uno se repite muy poco en un minuto. |
| 11 | Texto de la práctica | Palabras reales y comunes de `C.words`: 2 de cada 3 contienen un par débil, el resto son palabras comunes sin ellos | DeFulio y Ruopp. La mezcla evita que se aprenda de memoria. |
| 12 | Nombres visibles | «Transiciones lentas» para la vista y «Practicar transiciones lentas» para el botón | «Lentas» dice lo que se ve; sigue a «Transiciones fluidas». |
| 13 | Resultado de la práctica | Por cada par: «er: 265 ms hoy · 310 ms habitual». Se felicita sólo cuando un par sale de la lista: «er ya no es una transición lenta». Sin estrellas ni puntaje | Ruopp: en una sesión la mejora es de pocos milisegundos, y festejar ruido engaña. |
| 14 | Vista de transiciones lentas | Dos lugares: una línea en el resultado de cada lección y un gráfico en Progreso (ver «Ver las transiciones lentas») | Pedido. La vista sirve aunque nunca se use la práctica. |
| 15 | Qué muestra la vista | Hasta 10 pares, del más lento al menos lento: mediana en ms, el tiempo habitual del estudiante como referencia y cuántas muestras tiene | El número solo no dice nada; contra la referencia propia se entiende. |

Los números son iniciales y salen de constantes.

## Qué hay hoy y qué falta

- **Hay.** `transitions::analyze` ya da, por cada transición, el IKI, el tipo según los dedos y si
  fue medida ([transitions.rs](crates/klavaro-core/src/transitions.rs)). El `Tutor` de Progresión
  ya tiene los tiempos de cada letra ([strokes.rs](crates/klavaro-app/src/strokes.rs)).
- **Falta.** Guardar tiempos **por par**. Hoy `progression_attempts` sólo guarda totales por
  intento (medidas, fluidas, rollovers, mejor racha), así que no se sabe qué pares son lentos.
- **Modelo.** `KeyTracker` ([tracker.rs](crates/klavaro-core/src/tracker.rs)) y las tablas
  `key_errors` y `key_times` ([keys.rs](crates/klavaro-store/src/keys.rs)) hacen lo mismo para las
  teclas. La pantalla de Progreso ya grafica métricas por tecla (`Metric::Errors`,
  `Metric::TouchTimes`, `Chart::per_key` en [progress.rs](crates/klavaro-app/src/progress.rs)).

Una nota sobre lo que ya está hecho: en las transiciones fluidas, la misma tecla dos veces
cuenta como «mismo dedo» y recibe el umbral más flojo (factor 1,25). Feit y Dhakal encuentran
que las letras repetidas son de las transiciones más rápidas. No se toca en este plan; conviene
revisarlo aparte.

## Diseño técnico

### `klavaro-core`: módulo `pairs`

Sin dependencias de iced, probado con tiempos sintéticos.

```rust
pub struct PairEntry {
    pub from: char,
    pub to: char,
    pub kind: TransitionKind,
    samples: Vec<f64>,      // últimos IKI válidos, en segundos
    pub typed: u32,         // veces que se llegó al par con la primera letra bien
    pub wrong: u32,         // veces que la segunda letra salió mal
}

pub struct PairTracker { /* entradas por par */ }

pub struct WeakPair {
    pub from: char,
    pub to: char,
    pub median: Duration,   // sin ajustar: es lo que se muestra
    pub excess: f64,        // tiempo ajustado / tiempo habitual
    pub samples: u32,
    pub error_rate: f64,
}
```

- `PairTracker::record(from, to, kind, iki)` y `record_error(from, to)`.
- `PairTracker::typical() -> Option<Duration>`: la mediana de los tiempos ajustados, que es la
  referencia de las decisiones 7 y 15.
- `PairTracker::weak(frequencies, limit) -> Vec<WeakPair>` aplica las decisiones 5 a 9. La vista
  pide 10; la práctica, 4.
- `frequencies(words) -> mapa par → cantidad`, calculado una vez con la lista de palabras.
- `generators::pairs::generate(words, weak, chars, rng) -> String` arma el texto de la decisión
  11 con el largo de la decisión 1. Un par con menos de 5 palabras en la lista no se elige para
  practicar, pero sí se muestra en la vista.
- `Slot::target_chars` pasa a apoyarse en una función libre `target_chars(meta_wpm)`, para usar
  la misma cuenta sin tener un `Slot`.

### `klavaro-store`

Esquema 5, una tabla nueva (la migración sólo agrega):

```sql
CREATE TABLE IF NOT EXISTS pair_times (
    keyboard TEXT    NOT NULL,
    pair     TEXT    NOT NULL,
    kind     TEXT    NOT NULL,
    samples  TEXT    NOT NULL,
    typed    INTEGER NOT NULL,
    wrong    INTEGER NOT NULL,
    PRIMARY KEY (keyboard, pair)
);
```

Por teclado, como `key_times`: el tipo de transición depende de la distribución. Lectura y
guardado al lado de los de las teclas, en [keys.rs](crates/klavaro-store/src/keys.rs).

### `klavaro-app`: alimentar el rastreo

Al terminar una lección de Progresión (y una práctica de transiciones), el `Tutor` recorre el
resultado de `analyze` y carga el `PairTracker`. El perfil lo guarda junto con el rastreo de
teclas.

### `klavaro-app`: ver las transiciones lentas

- **En el resultado de la lección** ([gui/sections.rs](crates/klavaro-app/src/gui/sections.rs)),
  debajo de las transiciones fluidas, una línea con hasta 5 pares y sus tiempos:
  «Transiciones lentas: er 310 ms · th 280 ms · io 265 ms». Usa el rastreo ya actualizado con esa
  lección. Sin pares débiles, la línea no aparece.
- **En Progreso** ([progress.rs](crates/klavaro-app/src/progress.rs)), una métrica nueva sólo
  para Progresión, `Metric::SlowTransitions`, con el mismo gráfico de barras por tecla que ya
  existe: una barra por par (hasta 10), la altura es la mediana en ms y una línea marca el tiempo
  habitual del estudiante, donde hoy va la meta. Al pasar el mouse: muestras, errores y tipo de
  transición («mismo dedo», «misma mano», «otra mano»).
- **Sin datos suficientes**, el gráfico dice cuántas lecciones faltan, en lugar de quedar vacío:
  «Todavía no hay suficientes transiciones medidas».
- La vista es de lectura: no depende de la práctica y llega antes (Fase 3).

### `klavaro-app`: practicar (Fase 5, condicional)

- **Ejercicio.** `ExerciseSpec` gana una variante para esta práctica; `exercise::generate` llama
  a `generators::pairs`. Propuesta: no agregar un `Module` nuevo, sino una práctica de
  Progresión sin `Slot`, que no escribe en `progression_attempts` ni en `sessions`. A confirmar
  al implementar, viendo cuánto código asume que Progresión siempre tiene `Slot`.
- **Durante la práctica.** El destello y la racha de transiciones fluidas quedan activos, con la
  meta de la sección en curso. Sin límite de tiempo.
- **Entrada.** Botón «Practicar transiciones lentas» en el resultado de la lección, al lado de
  «Practicar teclas débiles», y junto al gráfico de Progreso. Sólo se muestra si hay al menos 2
  pares débiles con palabras para practicar. Antes de empezar se listan: «Vas a practicar:
  er · th · io».
- **Resultado.** Lo de la decisión 13, precisión y velocidad, y dos botones: repetir (con pares
  recalculados) y volver.
- **Textos.** Claves nuevas en `locales/app/C.ftl` y `es.ftl`.

## Sin romper lo que ya funciona

- **`Session` y `transitions::analyze` no cambian.** El `PairTracker` sólo lee su resultado.
- **Puntaje, estrellas y avance siguen igual.** La práctica no guarda intentos de lección.
- **«Practicar teclas débiles» no cambia.** Son dos botones y dos rastreos separados.
- **Las métricas actuales de Progreso no cambian.** Se agrega una, sólo en Progresión.
- **Migración aditiva.** El esquema 5 sólo crea una tabla. Se prueba abriendo una base con
  esquema 4.
- **Si faltan datos, no se ofrece.** Sin pares débiles suficientes no hay línea ni botón; nunca
  un texto vacío ni un error visible.
- **Pruebas.** Todas las actuales pasan sin cambios en cada fase (`cargo test`, `clippy`, `fmt`).

## Fase 1: tiempos por par

### Tareas

- Módulo `pairs` con `PairEntry`, `PairTracker`, `record` y `record_error`.
- Esquema 5 y guardado por teclado.
- El `Tutor` de Progresión carga el rastreo al terminar cada lección.
- `KLAVARO_KEYLOG=1` agrega la lista de los 10 pares más lentos, para calibrar.

### Criterios de aceptación

- Después de una lección, los pares escritos tienen muestras guardadas y sobreviven a reiniciar.
- Una transición con error, Backspace, autorrepetición o método de entrada no suma muestra.
- Un IKI mayor a 1 s, un par con espacio o con mayúscula y una letra repetida no suman muestra.
- Una base con esquema 4 se abre y migra sin perder nada.

### Resultado

Hecha, salvo probarla escribiendo una lección real.

- `klavaro_core::pairs` tiene `PairEntry` y `PairTracker`. `record_exercise(texto, letras)` carga
  una lección entera con las mismas `Letter` que recibe `transitions::analyze`.
- La tabla `pair_times` guarda también el tipo de transición (`kind`), porque el guardado no
  conoce los dedos. `TransitionKind` ganó `code` y `from_code` para eso.
- Un par cuya segunda letra salió mal cuenta como error aunque después se corrija. Si la que
  salió mal es la primera, el par no cuenta.
- Un par escrito bien pero sin tiempo válido (pausa de más de 1 s, método de entrada,
  autorrepetición) suma a «escritos» y no deja muestra.
- El `Tutor` carga los pares al terminar la lección y la app los guarda ahí mismo, sólo en la
  Progresión.
- `KLAVARO_KEYLOG=1` lista, después de la tabla por letra, los 10 pares más lentos acumulados
  con al menos 6 muestras: mediana, mediana ajustada, tipo, muestras y errores.

Probada con una lección real en Dvorak (noviembre de 2026): 322 transiciones medidas, los tipos
coinciden con la distribución y la mediana de `th` coincide con la tabla por letra.

Un ajuste que salió de esa lección: los pares con mayúscula quedan afuera. De las seis muestras
de `th`, las tres «Th» de inicio de oración tardaron 167, 172 y 216 ms y las tres «th», 92, 112
y 122 ms. Lo que frena es el Shift, y eso solo ponía a `th` primero en la lista. Las muestras de
«Th» guardadas antes del cambio se van solas a medida que entran nuevas.

## Fase 2: elegir los pares

### Tareas

- `PairTracker::typical` y `PairTracker::weak`, con el factor por tipo, la frecuencia y los
  errores.
- Pruebas con datos sintéticos, una por fila:

| Situación | ¿Es débil? |
|---|---|
| Par lento, 10 muestras, frecuente | Sí, primero |
| Par lento, 3 muestras | No |
| Par del mismo dedo, lento sólo sin ajustar | No |
| Par rápido, 20 % de errores | Sí |
| Dos pares igual de lentos, uno más frecuente | El frecuente antes |
| Par que era lento y cuyas últimas 30 muestras son rápidas | No |

### Criterios de aceptación

- `cargo test -p klavaro-core` cubre la tabla.
- Con el rastreo vacío o con pocas muestras, `weak` devuelve una lista vacía.

### Resultado

Hecha.

- `PairTracker::typical` da el tiempo habitual: la mediana de los tiempos ajustados de los pares
  con 6 muestras o más. Necesita al menos **8 pares medidos** (`MIN_PAIRS`); antes de eso no hay
  referencia y ningún par sale como lento.
- `PairTracker::weak(frecuencias, límite)` devuelve `WeakPair`, que además de lo previsto dice
  por qué es débil (`slow`, `error_prone`). `median` y `excess` son opcionales: un par que siempre
  sale mal no tiene tiempos y es débil igual, aunque todavía no haya tiempo habitual.
- El orden es `gravedad × (1 + frecuencia)`, donde la gravedad es cuánto pasa el par de su
  umbral: `exceso / 1,25` o `errores / 15 %`, lo que sea mayor. Con la frecuencia contada sobre
  todo `C.words`, entre pares débiles casi siempre manda la frecuencia.
- **Los errores viejos se olvidan.** Los tiempos ya se renovaban solos (últimas 30 muestras),
  pero las cuentas de «escritos» y «fallados» eran de por vida. Ahora, al pasar de 60 veces
  escritas, las dos se reducen a la mitad: la proporción queda igual y lo nuevo pesa más.
- **Un solo error no alcanza.** Con datos reales, `or` salía como «se falla» por 1 error en 6
  intentos (17 %) siendo de los pares rápidos. Ahora hacen falta al menos 2 errores
  (`MIN_ERRORS`) además del 15 %.
- `frequencies(palabras)` cuenta los pares de una lista de palabras con la misma regla de
  `pair_key` (sin mayúsculas ni letras repetidas).
- `KLAVARO_KEYLOG=1` ahora agrega el tiempo habitual y la lista de pares débiles con su exceso,
  sus errores, el motivo y la frecuencia.

## Fase 3: ver las transiciones lentas

### Tareas

- Línea «Transiciones lentas» en el resultado de la lección.
- `Metric::SlowTransitions` en Progreso, con la línea del tiempo habitual y el detalle al pasar
  el mouse.
- Textos en `C.ftl` y `es.ftl`.

### Criterios de aceptación

- Después de unas lecciones, el resultado lista los pares lentos con sus tiempos.
- El gráfico muestra hasta 10 pares ordenados y la referencia propia.
- Sin datos suficientes, la línea no aparece y el gráfico lo explica.
- Las demás métricas de Progreso se ven igual que antes.

## Fase 4: mirar los datos y decidir

Con la vista hecha, se usa la app dos semanas antes de construir la práctica.

### Tareas

- Revisar el gráfico y `KLAVARO_KEYLOG=1`: ¿los pares lentos coinciden con los que se sienten
  lentos?
- Ajustar las constantes de las decisiones 5, 7 y 8.
- Anotar acá qué pares salieron, si se repiten de una semana a la otra y si son frecuentes.

### Criterios de aceptación

- Los pares del mismo dedo no dominan la lista.
- Queda escrita la decisión sobre la Fase 5:
  - **Se hace** si los pares lentos son siempre los mismos y son frecuentes: hay algo que
    entrenar.
  - **No se hace** si la lista cambia sola de una semana a la otra o son pares raros: las
    lecciones ya lo resuelven.

## Fase 5: practicar (condicional)

Sólo si la Fase 4 lo justifica. Es la parte más cara (generador, variante de ejercicio, pantalla
de resultado) y la que menos respaldo tiene en la evidencia.

### Tareas

- `generators::pairs::generate` y `target_chars(meta_wpm)`.
- Variante de `ExerciseSpec`, los dos botones, la lista de pares antes de empezar y el resultado.

### Criterios de aceptación

- El texto mide `meta_wpm × 5` caracteres, con una tolerancia de una palabra; a la meta de la
  sección dura alrededor de un minuto.
- Al menos 2 de cada 3 palabras contienen un par débil, cada par elegido aparece y todas son
  palabras de la lista.
- La misma semilla da el mismo texto.
- Con menos de 2 pares débiles practicables, el botón no aparece.
- Hacer la práctica no cambia estrellas, lecciones aprobadas ni el historial de sesiones.
- Sólo se felicita cuando un par sale de la lista.
- Después de dos semanas de uso, queda anotado si la velocidad en las lecciones mejoró. Si no,
  la práctica se replantea; la vista se queda igual.

## Fuentes

- Feit, Weir y Oulasvirta (2016), *How We Type*:
  <https://userinterfaces.aalto.fi/how-we-type/resources/HowWeType_CHI16.pdf>
- Dhakal, Feit, Kristensson y Oulasvirta (2018), *Observations on Typing from 136 Million
  Keystrokes*: <https://www.pokristensson.com/pubs/DhakalEtAlCHI2018.pdf>
- DeFulio, Crone-Todd, Long, Nuzzo y Silverman (2011), *Using Words Instead of Jumbled
  Characters…*: <https://onlinelibrary.wiley.com/doi/10.1901/jaba.2011.44-921>
- Pinet, Zielinski, Alario y Longcamp (2022), *Typing Expertise in a Large Student Population*:
  <https://doi.org/10.1186/s41235-022-00424-3>
- Ruopp, Williams, Gach, Baese-Berk y Greenhouse (2026), *Motor Automaticity in Natural Keyboard
  Typing* (preprint): <https://pmc.ncbi.nlm.nih.gov/articles/PMC13278139/>
- Salthouse (1986), *Effects of Practice on a Typing-Like Keying Task*:
  <https://www.sciencedirect.com/science/article/abs/pii/0001691886900685>
