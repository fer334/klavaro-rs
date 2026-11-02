# Arquitectura

```
klavaro-app ──► klavaro-i18n
     │  └─────► klavaro-data ──► klavaro-core
     │  └─────► klavaro-store ─► klavaro-core
     └────────────────────────► klavaro-core
```

## Reglas

1. **`klavaro-core` no hace E/S ni conoce la interfaz.** No lee archivos, no consulta el
   reloj ni usa azar global. El instante de cada tecla (`Duration`) y el generador
   (`RandomSource`) se pasan por parámetro, así todas las pruebas son deterministas.
2. **`klavaro-data` trabaja sobre `&str`.** Interpreta y genera los formatos del original; leer y
   escribir el disco es responsabilidad de quien lo llama.
3. **Sólo `klavaro-app` conoce iced.** La interfaz traduce eventos de teclado a
   `session::Key`, pinta según `session::CharState` y muestra `goals::Comment` traducidos.
4. **Los textos visibles no viven en el dominio.** El core devuelve claves (`Comment`,
   `Rejection`…) que la capa de idioma convierte en texto.
5. **Errores tipados por crate**; nada de `unwrap()`/`expect()` fuera de las pruebas.

## Flujo de un ejercicio

1. La app genera el texto con `generators::{basic, adaptability, velocity, fluidity}`.
2. Crea una `Session` y le pasa cada tecla con `press(key, instante)`.
3. Cada `Feedback` indica qué pintar, si sonar y qué registrar en el `KeyTracker`.
4. `Feedback::Finished(stats)` trae las métricas; `Goals::comment` elige el comentario y,
   en Fluidez, `Ranking::insert` decide si entra al Top 10.
5. `profile.rs` (app) guarda la sesión en el historial con `klavaro-store`. Como el original,
   una sesión de Fluidez con menos de 500 caracteres no se registra.

## Persistencia

- `config.toml` se lee con tolerancia: claves faltantes toman el valor por defecto y los
  valores fuera de rango se corrigen (`Config::sanitized`). Se escribe en un temporal y se
  renombra.
- `history.sqlite3` tiene las tablas `sessions` (equivale a `stat_*.txt`), `ranking`
  (Top 10 local por idioma), `scores` (`scores_fluid.txt`) y `meta`. El esquema se versiona
  con `PRAGMA user_version`.
- Los instantes son segundos desde 1970. Las fechas importadas del original (hora local) se
  toman sin corregir la zona.
- Si el disco falla, la app sigue con la configuración en memoria y un historial en memoria,
  y muestra el error.

## Rastreo por tecla

- `KeyTracker` (core) acumula errores y tiempos por tecla de cada teclado. Como en el
  original, se alimenta en todos los módulos menos el curso básico, se borran los errores al
  cumplir la meta de Adaptabilidad y, si hay muchos errores o teclas muy lentas, aparece el
  botón de la sesión especial, que genera palabras con las teclas débiles.
- Durante un ejercicio lo tiene el `Tutor`; la app lo recupera al salir de la pantalla y lo
  guarda en las tablas `key_errors` y `key_times` (esquema 2) al terminar cada ejercicio y al
  salir. Lo tecleado en un ejercicio sin terminar se pierde si se cierra la ventana.
- `accuracy.log_<teclado>` y `proficiency.log_<teclado>` se importan una vez, con su propia
  marca en `meta`, para que también lleguen a bases creadas antes del esquema 2. Del
  segundo sólo existe el promedio por tecla, que se carga como tres tiempos iguales (como
  `accur_init`).

## Preferencias

- `settings.rs` (app, sin iced) es el formulario: los números se editan como texto y se
  validan al guardar con los rangos de `Config::sanitized` (acepta coma decimal). Los
  campos inválidos se marcan y no se guarda nada hasta corregirlos.
- Se editan el nombre del Top 10, los párrafos de Fluidez, el inicio automático y la
  precisión, velocidad y fluidez de cada meta. Los niveles de los comentarios, la voz y el
  sonido siguen sólo en `config.toml` (la voz y el sonido todavía no están implementados).

## Progreso y Top 10

- `progress.rs` (app, sin iced) arma un `Chart` con las últimas `DATA_POINTS` (50) sesiones
  vía `History::sessions(&SessionFilter, …)`, filtradas como en `plot.c`: Básico por lección
  (0 = todas) y teclado, Adaptabilidad por teclado, Velocidad y Fluidez por idioma. Fluidez
  agrega los gráficos de fluidez y de puntaje (tabla `scores`). El eje vertical parte del
  rango del original y crece si un valor o la meta no entran.
- `plot.rs` (app, `gui`) lo dibuja con el `canvas` de iced, con los colores de `plot.h`, la
  meta como línea gris y la fecha y el valor del punto más cercano al mouse.
- Adaptabilidad suma los gráficos por tecla del original (errores y tiempo de pulsación), de
  barras, con el rastreo del teclado actual (`Chart::per_key`).
- La pantalla Top 10 muestra el ranking local del idioma de los textos; los puntajes externos
  (descarga y publicación por la web) no se implementan.

## Fidelidad con Klavaro 3.14

Las fórmulas y constantes se verificaron contra el código original en `upstream/`
(`tutor.c`, `accuracy.c`, `top10.c`, `basic.c`, `adaptability.h`, `fluidness.h`, `plot.h`):
`MAX_BASIC_LESSONS` 256, `MIN_CHARS_TO_LOG` 500, Adaptabilidad 4 × 22 palabras de hasta 9
letras, fluidez sin tope por intervalo (el 4000 del original es el tamaño de un arreglo) y
desviación con denominador `n − 1`. Para los gráficos, el original muestra las últimas 50
sesiones (`DATA_POINTS`).

## Datos

`assets/` contiene los datos del original (GPL-3.0-or-later): 77 distribuciones `.kbd`,
listas `.words` y `.paragraphs` por idioma, `basic_lessons.txt` y `fingers_position.txt`.
`crates/klavaro-data/tests/assets.rs` verifica que todo se lea y genere ejercicios.
