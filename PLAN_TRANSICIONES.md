# Plan: recompensa por transiciones fluidas

Un mecanismo de motivación para la **Progresión**: premiar las transiciones entre letras que son
**rápidas, correctas y coordinadas**, incluido el rollover (empezar la tecla siguiente antes de
soltar la anterior), sin premiar el rollover por sí solo.

La regla de fondo:

```text
transición fluida = letra anterior correcta ∧ letra actual correcta ∧ IKI rápido ∧ sin retención artificial
```

El rollover no es condición: una transición rápida sin rollover también cuenta. El rollover sólo se
informa en las estadísticas, para que el estudiante aprenda qué es sin intentar forzarlo.

Alcance: sólo los ejercicios de Progresión. Los demás módulos no cambian. Todo el cálculo vive en
`klavaro-core`, así que extenderlo a otros módulos más adelante es barato.

## Decisiones por confirmar

| # | Decisión | Valor propuesto | Por qué |
|---|---|---|---|
| 1 | Nombre visible | «Transiciones fluidas» y «racha» | «Fluidez» ya es un módulo y una métrica (`fluidity`, regularidad del ritmo). Reusar la palabra confunde en el resumen y en el historial. |
| 2 | Umbral de IKI | `IKI ≤ 0,8 × IKI de la meta`, con `IKI de la meta = 12 000 / meta_wpm` ms | La lección ya fija una meta de velocidad: a 60 WPM la meta pide 200 ms por letra y la transición fluida, 160 ms. Se ajusta solo con la sección. |
| 3 | Ajuste por tipo de transición | Factor por tipo: otra mano 0,9; misma mano, otro dedo 1,0; mismo dedo 1,25 | Las transiciones del mismo dedo son físicamente más lentas. Sale del `FingerMap` que ya existe. |
| 4 | Retención artificial | Retención de la tecla anterior > `max(1,5 × mediana de retenciones del ejercicio, 180 ms)` ⇒ no cuenta | Corta el truco de mantener teclas apretadas sin castigar al que tiene retenciones naturalmente largas. |
| 5 | Racha visible | Desde 5 transiciones fluidas seguidas | Menos que eso es ruido y distrae. |
| 6 | Feedback | Destello durante el ejercicio y resumen al final, siempre activos en la Progresión. Sin ajuste ni comparación de modos | Confirmado. Menos opciones, menos superficie para romper algo. |
| 7 | Comparación en el resumen | Contra el intento anterior de la **misma lección**; si no hay, contra el último intento de Progresión | Con el mismo texto y la misma meta, la comparación es justa. |

Los números son iniciales y salen de constantes, como en la Progresión.

## Qué se puede medir hoy y qué falta

Hoy la app sólo escucha `KeyPressed` ([gui.rs](crates/klavaro-app/src/gui.rs) `on_event`) y toma
el tiempo cuando procesa el mensaje (`self.clock.elapsed()` en `App::key`). Con eso hay IKI pero no
rollover ni retención.

iced 0.14 ya da lo que falta:

- `KeyPressed { physical_key, repeat, .. }` y `KeyReleased { physical_key, .. }`.
- `physical_key` permite unir cada liberación con su pulsación aunque el carácter dependa de
  Shift o de una tecla muerta.
- `repeat` permite descartar la autorrepetición, que hoy cuenta como pulsaciones.

Dos problemas a resolver en la Fase 1:

1. **Marca de tiempo.** El instante se toma en `update`, no al recibir el evento. Si iced entrega
   varios eventos en el mismo cuadro, los IKI cortos se distorsionan. Hay que tomar el instante en
   `on_event` con un reloj global (`OnceLock<Instant>`) y mandarlo dentro del mensaje.
2. **Método de entrada en macOS.** Las letras con tecla muerta llegan como `InputMethod::Commit`,
   sin tecla física. Esas transiciones (antes y después de la letra) se marcan como «sin medir» y
   no cuentan ni a favor ni en contra. En los textos en inglés de la Progresión casi no aparecen.

## Diseño técnico

### `klavaro-core`: módulo `transitions`

Un módulo nuevo, sin dependencias de iced, probado con tiempos sintéticos.

```rust
pub struct Stroke { pub down: Duration, pub up: Option<Duration> }

pub enum TransitionKind { OtherHand, SameHand, SameFinger, Unknown }

pub struct Transition {
    pub at: usize,              // índice de la letra que cierra la transición
    pub iki: Duration,
    pub rollover: bool,         // la anterior se soltó después de bajar ésta
    pub held_too_long: bool,
    pub kind: TransitionKind,
    pub fluent: bool,
}

pub struct TransitionSummary {
    pub measured: u32,
    pub fluent: u32,
    pub rollovers: u32,         // sólo estadística
    pub best_streak: u32,
}

pub struct TransitionTracker { /* config, strokes por posición, racha actual */ }
```

- `TransitionTracker::new(config)` con la meta de la lección y el `FingerMap` del teclado.
- `down(pos, at)` cuando la `Session` acepta una letra **correcta a la primera**; devuelve si la
  transición que cierra fue fluida, para el destello.
- `up(pos, at)` cuando se suelta la tecla de esa letra.
- Un error, un Backspace o una letra sin medir **cortan la racha** y la transición que los toca no
  cuenta.
- La retención se evalúa al soltar la tecla anterior, así que una transición puede pasar de fluida
  a no fluida un instante después. El destello usa la evaluación provisional (sin retención); el
  resumen usa la definitiva. La mediana de retenciones se calcula con las del propio ejercicio,
  después de las primeras 10.
- `summary()` al terminar.

`Session` no cambia: el tracker vive al lado y se alimenta con su `Feedback`.

### `klavaro-app`

- **Eventos** ([gui.rs](crates/klavaro-app/src/gui.rs)): `on_event` también traduce `KeyReleased`
  a `Message::KeyUp(Physical, Duration)`, y `Message::Key` lleva `Physical` y el instante. Las
  pulsaciones con `repeat` se ignoran sólo para el tracker.
- **Tutor** ([tutor.rs](crates/klavaro-app/src/tutor.rs)): si el módulo es Progresión, el `Tutor`
  tiene un `TransitionTracker` y un mapa `Physical → posición` de las teclas apretadas. En
  `press`, `Feedback::Correct` llama a `down`; `KeyUp` llama a `up`. `finish` guarda el
  `TransitionSummary` en el `Outcome`.
- **Destello**: `SegmentStyle::Flash` para letras con transición fluida en los últimos 250 ms. El
  tutor guarda `(posición, instante)` de los destellos vivos y `segments()` los pinta. Mientras
  haya uno vivo, `subscription` agrega `window::frames()` para redibujar; sin destellos no hay
  tick. Color: un tono del `Palette` que funcione en claro y oscuro, distinto de `correct` y de
  `cursor`.
- **Racha**: desde 5, una línea discreta bajo el texto: «Racha: 6 transiciones fluidas».
- **Resumen** ([gui/sections.rs](crates/klavaro-app/src/gui/sections.rs), resultado de la
  lección): «Transiciones fluidas: 42 de 118 (36 %)», la mejor racha y la comparación:
  - «18 transiciones fluidas más que la vez anterior, con la misma precisión» si subió y la
    precisión no bajó más de 1 punto.
  - Si subió pero la precisión bajó, no se felicita: «Más transiciones fluidas, pero bajó la
    precisión».
  - Sin intento anterior, sólo el número.
  - En una línea secundaria, el rollover como dato: «Rollover: 23 % de las transiciones».
- **Textos**: claves nuevas en `locales/app/C.ftl` y `es.ftl`.

### Guardado

Esquema 4 de [history.rs](crates/klavaro-store/src/history.rs), columnas nuevas y opcionales en
`progression_attempts` (los intentos viejos quedan en `NULL`):

```sql
ALTER TABLE progression_attempts ADD COLUMN transitions_measured INTEGER;
ALTER TABLE progression_attempts ADD COLUMN transitions_fluent   INTEGER;
ALTER TABLE progression_attempts ADD COLUMN rollovers            INTEGER;
ALTER TABLE progression_attempts ADD COLUMN best_streak          INTEGER;
```

Más una consulta `previous_transitions(language, section, lesson)` para la comparación.

Las transiciones **no** cambian puntaje ni estrellas: son una recompensa aparte.

## Sin romper lo que ya funciona

Reglas que valen para todas las fases y se revisan en cada merge:

- **`Session` no cambia.** La evaluación de teclas, errores, Backspace, `Stats`, WPM, precisión,
  `fluidity`, puntaje y estrellas siguen exactamente igual. El `TransitionTracker` sólo observa el
  `Feedback` que ya devuelve la `Session`.
- **Los demás módulos no se enteran.** El tracker, el destello y la racha existen sólo si el módulo
  es Progresión. `KeyUp` se descarta en cualquier otra pantalla antes de llegar al `Tutor`.
- **La entrada de teclas sigue igual.** Agregar la tecla física y el instante a `Message::Key` no
  cambia qué teclas se traducen ni cómo. La autorrepetición sigue llegando a la `Session` como hoy;
  sólo el tracker la ignora. El camino del método de entrada (`Typed`) no se toca.
- **El reloj nuevo da los mismos tiempos.** Si la Fase 1 cambia de dónde sale el instante, el WPM
  de un mismo tecleo tiene que coincidir con el de hoy (prueba con tiempos sintéticos) para no
  mover estrellas ni metas ya ganadas.
- **Migración aditiva.** El esquema 4 sólo agrega columnas opcionales. No borra, no renombra y no
  reescribe intentos. Se prueba abriendo una base con esquema 3.
- **Si algo falla, se apaga solo.** Si faltan liberaciones o tiempos, la transición queda «sin
  medir». Nunca hay pánico ni error visible por esto, y la lección termina igual.
- **Pruebas.** Todas las pruebas actuales pasan sin cambios en cada fase (`cargo test`, `clippy`,
  `fmt`). Se agrega una prueba de que un tecleo de Progresión da el mismo `Outcome` (estadísticas,
  puntaje, estrellas) con y sin el tracker.

## Fase 1: captura de pulsaciones y liberaciones

### Tareas

- Reloj global y marca de tiempo en `on_event`.
- `KeyReleased`, `physical_key` y `repeat` hasta el `Tutor`.
- Un registro de depuración (detrás de una variable de entorno) con `down`/`up` por letra.

### Criterios de aceptación

- Escribiendo un texto de Progresión, cada letra tiene `down` y `up`, salvo las de método de
  entrada.
- Con Shift, la liberación de la letra se une igual a su pulsación.
- La autorrepetición no genera transiciones.
- Medido en macOS y Linux: diferencias de IKI contra un registro externo menores a 5 ms.

### Resultado

Hecha. `KLAVARO_KEYLOG=1` imprime, al terminar una lección, una tabla por letra con estado,
bajada, soltado, retención, IKI y rollover.

Dos lecciones reales en macOS (noviembre de 2026):

- Todas las letras escritas con tecla física tienen bajada y soltado. Retenciones típicas de
  80–200 ms; IKI de 50–80 ms en pares como `th`, `he`, `e␣`; rollover en cerca de la mitad de
  las transiciones rápidas y casi nunca en las lentas.
- Mantener una tecla más de ~250 ms dispara la **autorrepetición del sistema**, que escribe una
  letra de más (como siempre). Esa letra queda sin tiempos y marcada en la tabla. La Fase 2 tiene
  que cortar la racha ahí y no contar las transiciones que la tocan.
- Hay IKI de ~22 ms con rollover que terminan en error (letras invertidas). La condición de
  «ambas letras correctas» ya los excluye.
- Las letras corregidas guardan los tiempos de la última pulsación; la Fase 2 no las cuenta.

Falta medir en Linux.

## Fase 2: cálculo en `klavaro-core`

### Tareas

- Módulo `transitions` con las reglas de arriba y las constantes de la tabla.
- Pruebas con secuencias sintéticas, una por fila de esta tabla:

| Situación | ¿Fluida? |
|---|---|
| Rollover, IKI 90 ms, letras correctas | Sí |
| Rollover, IKI 240 ms | No |
| Sin rollover, IKI 85 ms | Sí |
| Rollover, letra incorrecta | No |
| Rollover, tecla anterior retenida artificialmente | No |
| Transición después de un Backspace | No (corta la racha) |
| Mismo dedo, IKI apenas sobre el umbral general | Sí, por el factor del tipo |

### Criterios de aceptación

- `cargo test -p klavaro-core` cubre la tabla, la racha y la mediana de retenciones.

### Resultado

Hecha, con un cambio de forma: en lugar de un rastreador que se alimenta tecla por tecla, el
módulo `klavaro_core::transitions` es una función pura, `analyze(letras, meta_wpm)`, que recibe el
estado, los tiempos y el dedo de cada letra. Es más simple de probar, y con textos de unos cientos
de letras se puede llamar en cada tecla para la racha en vivo de la Fase 4.

- Una transición sin tiempos en alguna letra (autorrepetición, método de entrada) no se mide y
  corta la racha.
- La mediana de retenciones se usa desde 10 muestras; con menos, el límite es 270 ms.
- El espacio (pulgares) cuenta como «otra mano»; la misma tecla dos veces, como «mismo dedo».
- `KLAVARO_KEYLOG=1` ahora agrega la columna «fluida» y los totales, para calibrar con datos
  reales antes de mostrar nada en la interfaz.

## Fase 3: resumen y guardado

### Tareas

- `TransitionTracker` en el `Tutor` de Progresión y `TransitionSummary` en el `Outcome`.
- Esquema 4 y la consulta del intento anterior.
- Bloque de transiciones en el resultado de la lección, con la comparación y el dato de rollover.

### Criterios de aceptación

- Repetir una lección muestra la diferencia con el intento anterior.
- Bajar la precisión nunca produce un mensaje de felicitación.
- Una base de datos con esquema 3 se abre y migra sin perder intentos.

### Resultado

Hecha.

- El resultado de una lección de Progresión muestra «Transiciones fluidas: N de M (P %) · mejor
  racha: R», la comparación si mejoró y el rollover como dato, con su explicación.
- La comparación es contra el último intento **con transiciones medidas** de la misma lección
  (cantidades) o, si no hay, contra el último de la Progresión (puntos porcentuales). Se lee
  antes de guardar el intento nuevo.
- Si la precisión bajó más de 1 punto, dice «pero bajó la precisión» en lugar de felicitar. Si
  no mejoró, no dice nada.
- La migración agrega las columnas con `ALTER TABLE` sólo si faltan; abrir la base otra vez no
  hace nada. Los intentos viejos quedan en `NULL` y no cuentan para comparar.
- Límite conocido: en la última transición la tecla anterior suele soltarse después de terminar
  la lección, así que su rollover no se cuenta (sí cuenta si fue fluida).

## Fase 4: destellos y racha durante el ejercicio

### Tareas

- `SegmentStyle::Flash`, la animación de 250 ms y el tick sólo mientras haya destellos.
- Indicador de racha.

### Criterios de aceptación

- En la Progresión, cada transición fluida destella y se apaga sola.
- En los demás módulos, durante el ejercicio no cambia nada respecto de hoy.
- Sin destellos vivos la app no redibuja por su cuenta (medido con el uso de CPU en reposo).
- Se lee bien en tema claro y oscuro.

### Resultado

Hecha.

- Con cada letra escrita, el tutor analiza lo escrito hasta ahí con `transitions::analyze`. Si
  esa letra cerró una transición fluida, la letra destella. El destello es un fondo verde (el
  color de «bien») que se apaga en 250 ms, en cuatro niveles. La retención de la tecla anterior
  todavía puede no conocerse; en ese caso decide el IKI y el resumen final usa la evaluación
  completa.
- `window::frames()` se suscribe sólo mientras hay un destello encendido, y cada cuadro
  (`FlashTick`) vuelve sin hacer nada más que redibujar.
- La racha aparece en el pie de la lección, junto a la meta y el tiempo, desde 5 transiciones
  fluidas seguidas. Un error, una letra sin tiempos o Backspace la cortan.
- Los demás módulos no destellan ni muestran racha.

## Fase 5: calibración

### Tareas

- Revisar con intentos reales qué porcentaje de transiciones sale fluida por sección. Objetivo:
  entre 20 % y 60 %, para que haya margen de mejora sin que sea imposible. Ajustar los factores.

### Criterios de aceptación

- Un informe corto en este archivo con los números y los umbrales finales.

## Orden y dependencias

Fase 1 → Fase 2 → Fase 3 → Fase 4 → Fase 5. La Fase 2 puede hacerse en paralelo con la 1, porque no
depende de iced. Cada fase se puede mergear sola; la 3 ya entrega el valor principal (recompensa al
final).

## Riesgos

- **Precisión de tiempos.** Si el bucle de eventos agrupa teclas, el rollover medido puede no ser
  real. La Fase 1 lo mide antes de construir encima.
- **Distracción.** Los destellos pueden empeorar la precisión. Por eso son cortos (250 ms), de un
  color suave y sin sonido. Si en la Fase 5 se nota, se acorta el destello antes de pensar en
  apagarlo.
- **Umbrales fijos injustos.** El factor por tipo de transición es una aproximación. Si la Fase 5
  muestra sesgos, el siguiente paso es un umbral por bigrama con los tiempos guardados del usuario.
- **Teclados y sistemas.** En Wayland y con algunos métodos de entrada las liberaciones pueden
  llegar tarde o no llegar. Una pulsación sin liberación cuenta como «retención desconocida»: la
  transición se evalúa sólo por IKI.
