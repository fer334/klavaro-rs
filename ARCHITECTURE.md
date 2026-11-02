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

Las fórmulas y constantes vienen de `tutor.c`, `accuracy.c`, `top10.c` y los módulos del
original. Valores por confirmar contra `upstream/`:

- `ranking::MIN_CHARS` (`MIN_CHARS_TO_LOG`).
- Párrafos y palabras por ejercicio de Adaptabilidad (`LINES`, `WORDS`) y `MAX_WORD_LEN`.
- El denominador exacto de la desviación estándar en `metrics::fluidity`.
