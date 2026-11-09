# Plan: errores aislados en la Progresión

Antes, en la Progresión, un error "contagiaba" a todo lo que venía después: hasta volver con
Backspace, cada letra tecleada quedaba roja aunque estuviera bien, y había que corregir para
terminar. Ahora un error marca **sólo la letra donde ocurrió**.

## Reglas

| Situación | Qué pasa |
|---|---|
| Tecla correcta | La letra queda bien y el cursor avanza. |
| Tecla incorrecta | La letra queda en rojo, suma un error y el cursor avanza. La siguiente se evalúa sola. |
| Backspace | Vuelve una letra atrás. Si se reescribe bien una letra que estuvo mal, queda como corregida. |
| Ctrl+W | Vuelve al comienzo de la palabra. |
| Corregir | Es opcional. El error original sigue contando para la precisión. |
| Última letra | El ejercicio termina al teclearla, bien o mal. |

No hay reglas especiales para el espacio: una letra omitida o de más corre el texto hasta que se
corrija con Backspace.

## Métricas y estrellas

- Precisión: `100 × (1 − errores / pulsaciones)`, sin cambios.
- Velocidad: neta, `12 × letras bien escritas al final / segundos`. Las corregidas cuentan como bien;
  las que quedaron mal, no. Dejar un error sale más caro en velocidad, y corregirlo cuesta tiempo.
- Tiempo máximo: `time_limit(letras bien escritas, lección)`. 5 estrellas sigue equivaliendo a
  terminar dentro de ese tiempo. Durante la lección baja con cada letra que queda mal.

## Cambios

| Lugar | Cambio |
|---|---|
| `klavaro-core/src/lib.rs` | `allows_correction` pasa a `Module::evaluation()` con `Forward`, `Correction` (Fluidez, sin cambios) y `Free` (Progresión). |
| `klavaro-core/src/session.rs` | `eval_free`, `Session::wrong_letters()` y sus pruebas. |
| `klavaro-core/src/progression.rs` | `LessonResult::new(slot, correct, stats)` usa la velocidad neta. |
| `klavaro-app/src/tutor.rs` | `Tutor::correct_letters()` para el resultado y el tiempo máximo. |
| `klavaro-app/src/gui.rs` | Tiempo máximo efectivo durante la lección. |
| `ARCHITECTURE.md`, `PLAN_PROGRESION.md` | Reglas nuevas. |

## Impacto

- Fluidez no cambia.
- El rastreo de teclas débiles deja de recibir errores falsos: sólo anota las letras realmente mal.
- Historial: no hace falta migrar. Los intentos viejos se midieron con la regla anterior, así que
  los gráficos de la Progresión mostrarán un salto en la precisión. Nadie pierde estrellas.
- Los topes de precisión de las estrellas (95 % / 90 %) se mantienen; revisarlos después de jugar.

## Pendiente

- Jugar varias lecciones en la app: errores sin corregir, corregidos, y el tiempo máximo bajando.
