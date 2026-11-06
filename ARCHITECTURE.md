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

## Progresión

Es una sección propia, con otra lógica que los cuatro módulos del original. Por eso
`Module::Progression` no está en `Module::ALL`, que alimenta el menú, las metas de Preferencias y
la importación.

- `progression.rs` (core) tiene las reglas: 17 secciones de 20 a 100 PPM, cada una con 30 lecciones.
  La sección lleva el nombre de la meta a la que llega, y cada lección pide 1/6 de PPM más que la
  anterior.
- **Tiempo y velocidad:** el tiempo máximo es `12 × caracteres / meta`. Una lección sólo termina con
  el texto bien escrito, así que la velocidad sale del largo y del tiempo, y 5 estrellas equivale
  exactamente a terminar dentro del tiempo. No se usan las PPM de `Stats`, que suben al errar varias
  veces la misma letra.
- **Estrellas:** 5 con `velocidad ≥ meta`, y 4, 3 y 2 con el 90, 80 y 70 %; terminar siempre da 1.
  Con menos de 95 % de precisión el máximo es 4, y con menos de 90 % es 2.
- **Desbloqueo:** 3 estrellas abren la lección siguiente. Aprobar una sección pide las 30 lecciones
  con 3 estrellas o más y la última con 5.
- **Examen:** cualquier sección que no quedó atrás, también una bloqueada, se puede superar jugando
  su lección 30 con 5 estrellas. Eso da por superadas esa sección y las anteriores, que se ven como
  «Saltada», sin regalar sus estrellas, y sus lecciones quedan abiertas en cualquier orden. La sección
  más alta se guarda en `meta` (`progression_exam_<idioma>`).
- `sections.rs` (app, sin iced) tiene la selección y `gui/sections.rs` las vistas. Las lecciones usan
  el `Tutor` con corrección, como Fluidez. El tiempo en pantalla se actualiza con cada tecla.
- Por ahora los ejercicios son sólo en inglés (`LESSONS_LANGUAGE`), sea cual sea el idioma de los
  textos de los otros módulos.
- El puntaje es el de `metrics::score`. Las sesiones de Progresión entran al historial, pero no
  pasan por el mínimo de caracteres ni entran al Top 10.

## Persistencia

- `config.toml` se lee con tolerancia: claves faltantes toman el valor por defecto y los
  valores fuera de rango se corrigen (`Config::sanitized`). Se escribe en un temporal y se
  renombra.
- `history.sqlite3` tiene las tablas `sessions` (equivale a `stat_*.txt`), `ranking`
  (Top 10 local por idioma), `scores` (`scores_fluid.txt`) y `meta`. El esquema se versiona
  con `PRAGMA user_version`.
- El esquema 3 suma `progression_attempts`, con un registro por intento. Los mejores resultados
  y el desbloqueo se calculan desde ahí, así un resultado peor nunca baja las estrellas. El
  progreso se asocia a la lección y no al texto: reemplazar un texto no hace perder estrellas.
  Reiniciar la Progresión borra sólo esta tabla.
- Los instantes son segundos desde 1970 en UTC y se muestran en la hora local (`chrono`). Las
  fechas de los `.txt` del original son locales y se pasan a UTC al importar; las bases
  importadas antes de esa corrección se arreglan una vez (`utc_times_once`).
- El original guarda el idioma por su nombre («Español»); al importar se pasa al código
  (`es`), y las bases ya importadas se corrigen una vez (`normalize_languages_once`).
- El Top 10 local del original (`ksc/local_<xx>.ksc`, binario) se importa una vez para los
  idiomas sin ranking propio.
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
- Se editan el nombre del Top 10, los párrafos de Fluidez, el inicio automático, el pitido
  y la precisión, velocidad y fluidez de cada meta. Los niveles de los comentarios siguen
  sólo en `config.toml`; el modo dictado se activa en el menú, como en el original.

## Dictado y pitido

- `speech.rs` (app, sin iced) decide qué decir, como `tutor_speak_char` y
  `tutor_speak_word`: el carácter en Básico y Adaptabilidad (espacio, intro, comillas… por
  su nombre traducido), la palabra siguiente en Velocidad, nada en Fluidez. El dictado del
  dedo a los 3 s del curso básico no está implementado.
- `audio.rs` lanza programas del sistema sin shell: `espeak-ng`/`espeak -v<idioma> -k1`
  (y `--punct` como el original), o `say` en macOS; el pitido usa el sonido de alerta del
  sistema. Un dictado nuevo corta el anterior; si un programa falla, no se reintenta.
- La interfaz dicta cuando cambia la posición (ejercicio y cursor), así un error no repite
  el dictado.

## Teclado virtual y dedos

- `hints.rs` (app, sin iced) da, para el carácter siguiente, la tecla, el dedo
  (`fingers_position.txt`) y el Shift de la otra mano si hace falta; espacio y Enter son de
  los pulgares y del meñique derecho.
- `keys.rs` (app, `gui`) dibuja el teclado con forma ISO y los colores por dedo del original
  (`KEYB_*`), resalta la tecla siguiente y el Shift, y debajo nombra el dedo y la mano. Se
  muestra en Básico y Adaptabilidad; el botón Teclado lo oculta (`show_keyboard`).
- Con el modo dictado, en Básico se dice el dedo tras 3 s sin avanzar, como
  `tutor_delayed_finger_tip`. El ejecutor de iced no tiene temporizadores: espera un hilo,
  a lo sumo uno a la vez, que se rearma con lo que falte si la posición cambió.
- No están las imágenes de las manos (`hands_*.png`) ni la ventana de teclado del menú con
  la introducción.

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
- En la Progresión, los gráficos se filtran por sección y lección (`75-03` en la columna `lesson`), con
  la meta de la lección como línea de referencia. Las estrellas por sección son barras armadas sin
  `Plot`, porque no salen del historial de sesiones.

## Fidelidad con Klavaro 3.14

Las fórmulas y constantes se verificaron contra el código original en `upstream/`
(`tutor.c`, `accuracy.c`, `top10.c`, `basic.c`, `adaptability.h`, `fluidness.h`, `plot.h`):
`MAX_BASIC_LESSONS` 256, `MIN_CHARS_TO_LOG` 500, Adaptabilidad 4 × 22 palabras de hasta 9
letras, fluidez sin tope por intervalo (el 4000 del original es el tamaño de un arreglo) y
desviación con denominador `n − 1`. Para los gráficos, el original muestra las últimas 50
sesiones (`DATA_POINTS`).

## Datos

`assets/C.progression` tiene un texto en inglés por lección, con su fuente y revisión. Lo genera
`tools/progression/` (ver su [README](tools/progression/README.md)) a partir de Wikipedia y Simple
English Wikipedia, con licencia CC BY-SA 4.0, y lo lee `klavaro_data::catalog`.

`assets/` contiene los datos del original (GPL-3.0-or-later): 77 distribuciones `.kbd`,
listas `.words` y `.paragraphs` por idioma, `basic_lessons.txt` y `fingers_position.txt`.
`crates/klavaro-data/tests/assets.rs` verifica que todo se lea y genere ejercicios.

El `build.rs` de `klavaro-app` los embebe en el binario con `include_str!` (unos 1,7 MB), así
el ejecutable funciona instalado en cualquier lugar. `KLAVARO_ASSETS` los reemplaza por los de
otra carpeta.

## Empaquetado

- `tools/package.sh [TRIPLE]` compila en release (LTO, `strip`) y arma en `dist/` los
  paquetes del sistema:
  - macOS: `.dmg` con `klavaro-rs.app` y el atajo a Aplicaciones (`hdiutil`; ícono `.icns`
    generado con `sips` e `iconutil`).
  - Linux: `.tar.gz` con `bin/` y `share/`, y `.deb` (`dpkg-deb`, dependencias calculadas con
    `dpkg-shlibdeps`).
  - Windows: instalador `.msi` (WiX 5, `packaging/windows/klavaro-rs.wxs`: Archivos de
    programa, acceso directo en el menú Inicio, actualización sobre la versión anterior) y el
    `.exe` portable.
- `packaging/` tiene el ícono del original (SVG, PNG de 256 px e `.ico`), el `.desktop`, el
  `Info.plist` y el `.wxs`.
- `.github/workflows/release.yml` corre el script en Linux, macOS (ARM e Intel) y Windows y,
  con una etiqueta `v*`, crea un borrador de release con los paquetes y `SHA256SUMS`.
- Aviso de versión nueva (`update.rs`, función `update`): al abrir, la app consulta
  `releases/latest` del repositorio fijado al compilar con `KLAVARO_RELEASES` (el workflow
  usa `github.repository`) y, si hay una versión mayor, muestra un aviso con el enlace. Los
  borradores no cuentan hasta publicarlos. Se desactiva en Preferencias.
- Pendiente: firma y notarización en macOS, firma, ícono del `.exe` y ventana sin consola en
  Windows, y Flatpak.
