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
