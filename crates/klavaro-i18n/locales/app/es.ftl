# SPDX-License-Identifier: GPL-3.0-or-later
# Textos propios de klavaro-rs en español. Ver C.ftl.

app-name = klavaro-rs
menu-interface-language = Interfaz:
menu-text-language = Textos:
menu-colors-dark = Colores oscuros
menu-colors-light = Colores claros
menu-shortcuts = Atajos: 1–4 abren un módulo · N progresión · P progreso · T Top 10 · S preferencias · Ctrl+A cambia los colores
tutor-new = Nuevo (Esc)
tutor-lesson-n = Lección { $lesson }
tutor-hint = { $done }/{ $total } · Esc: nuevo ejercicio · Ctrl+A: colores
tutor-hint-backspace = { $done }/{ $total } · Esc: nuevo ejercicio · Ctrl+A: colores · Backspace corrige
stats-seconds = { $value } s
stats-percent = { $value } %
stats-wpm-value = { $value } PPM
comment-too-many-errors = :-(
ps-invalid = :-P
notice-imported = Se importaron { $sessions } sesiones y las preferencias del Klavaro original.
notice-error = No se pudieron leer o guardar sus datos: { $error }
error-no-data = No se encontraron datos en { $dir }
practice-instructions = Copie cada línea y pulse Intro al final de cada una.

## Mensajes del original que es.po tiene sin traducir o marcados como dudosos.

app-tagline = Un tutor de mecanografía muy flexible y eficiente.
tutor-restart-tip = Pulse aquí para reiniciar el ejercicio. Atajo: [Ctrl-R]
tutor-use-backspace = Use la tecla de retroceso para corregir errores.
comment-adapt-try-again =
    Su precisión está por debajo del { $target }%...
    ¿Podría intentarlo de nuevo para mejorarla?
comment-adapt-improving =
    Lo está haciendo bien. Pero...
    ¿Podría llevar la precisión hasta el { $target }%?
comment-adapt-almost =
    Ya casi lo tiene, pero su precisión sigue por debajo del { $target }%.
    Inténtelo unas veces más o, si se está cansando, pase a otro tipo de ejercicio.
comment-velo-highway =
    Todavía lejos de la autopista. Puede hacerlo mejor...
    Intente llegar al menos a { $target } PPM.
comment-velo-faster =
    Lo está haciendo bien, pero necesita ir más rápido.
    Y no se olvide de la precisión. Intente llegar a { $target } PPM.
comment-velo-running =
    Bien. Ahora necesita empezar a correr.
    ¿Puede llegar a { $target } PPM?
comment-velo-professional =
    ¡Rápido! ¿Es usted profesional?
    Entonces, ¡intente llegar a { $target } PPM!
comment-velo-speed-racer = ¡Muy bien, Meteoro! ¿Le da miedo llegar a { $target } PPM?
comment-velo-kung-fu =
    ¡Maestría de kung-fu!
    ¿Puede volar a { $target } PPM?
progress-all-lessons = Todas las lecciones
progress-empty = Todavía no hay sesiones para graficar.
progress-sessions = Últimas { $count } sesiones · { $first } – { $last }
progress-goal = Meta: { $value }
ranking-place = #
tutor-special = Teclas débiles
tutor-special-on = Teclas débiles ✓
progress-keys = { $count } teclas · { $keyboard }
menu-settings = Preferencias
settings-name = Nombre para el Top 10:
settings-name-tip = Déjelo vacío para usar el nombre del usuario del sistema.
settings-paragraphs-full = Texto completo
settings-autostart = Abrir el último módulo al iniciar
settings-goals = Metas
settings-goals-tip = Precisión y fluidez en %, velocidad en PPM. Los niveles de los comentarios se editan en config.toml.
settings-defaults = Metas por defecto
settings-save = Guardar
settings-saved = Preferencias guardadas.
settings-invalid = Revise los campos en rojo: la precisión y la fluidez van de 0 a 100, la velocidad de 0 a 200.
menu-speech-tip = Modo dictado: dice cada tecla o palabra antes de teclearla (con { $engine }).
menu-speech-missing = El modo dictado necesita espeak o espeak-ng instalado (en macOS también sirve say).
settings-beep = Pitido al equivocarse
hint-left = { $finger } (mano izquierda)
hint-right = { $finger } (mano derecha)
hint-shift = Mayúsculas con la otra mano
notice-imported-sessions = Se importaron { $sessions } sesiones del Klavaro original.
progress-session-one = Última sesión · { $first }
settings-check-updates = Avisar cuando haya una versión nueva
update-available = Hay una versión nueva de klavaro-rs: { $version } (tiene la { $current }).
update-download = Descargar
update-install = Actualizar
update-installing = Descargando e instalando la versión { $version }…
update-failed = No se pudo actualizar: { $error }. Podés descargarla a mano.

## Progresión.

module-progression = Progresión
sections-back = Volver
sections-exam = Hacer el examen (E)
sections-exam-tip = Escriba un texto a { $goal } PPM con { $accuracy } % de precisión para abrir la sección siguiente, sin jugar las lecciones anteriores.
sections-exam-title = Examen: sección de { $section } PPM
sections-exam-failed = Necesita 5 estrellas para aprobar el examen. Puede reintentarlo todas las veces que quiera.
sections-skipped = Saltada
sections-skipped-rule = Superó esta sección con un examen. Sus lecciones siguen abiertas por si quiere ganar sus estrellas.
sections-locked-rule = Esta sección está bloqueada. Apruebe su examen, o la sección anterior, para seguir.
sections-stars = Estrellas
sections-gained = +{ $count } ★
sections-weak-keys = Teclas para practicar: { $keys }
sections-space = Espacio
sections-practice = Practicar teclas débiles
sections-overall = { $done }/{ $total } lecciones aprobadas · Mejor meta aprobada: { $best } · Meta final: { $goal }
sections-next-goal = Próxima meta: { $section } PPM
sections-completed-detail = Llegó a la meta de { $goal } PPM.
sections-continue = Seguir (Intro)
sections-milestone-keys = Esc: volver a las secciones
sections-reset = Reiniciar progresión
sections-reset-tip = Borra todas las estrellas de la Progresión. El historial y el Top 10 se conservan.
sections-reset-confirm = ¿Borrar todas sus estrellas?
sections-reset-yes = Sí, borrar
sections-reset-no = Cancelar
sections-reset-done = Se reinició la Progresión.
progress-all-sections = Todas
sections-credits = Créditos
sections-credits-intro = Los textos de estas lecciones son obra de los colaboradores de Wikipedia y Simple English Wikipedia, con licencia { $license }. Se recortaron y adaptaron, y se comparten con la misma licencia. Pulse una lección para abrir su artículo original.
sections-license = Licencia ({ $license })
sections-source = Texto: { $title } ({ $source }, { $license }), adaptado
sections-missing-keys = Este teclado no tiene: { $keys }
module-progression-tip = Suba de 20 a 100 PPM, lección a lección, ganando estrellas. Textos en inglés.
sections-section = Sección de { $section } PPM
sections-intro = Cada sección tiene 30 lecciones, y cada lección pide un poco más de velocidad que la anterior, hasta la meta de { $goal } PPM.
sections-keys = Flechas: moverse · Intro: abrir · E: examen (dentro de una sección) · Esc: volver
sections-locked = Bloqueada
sections-available = Disponible
sections-approved = Sección aprobada
sections-lessons-passed = { $done }/{ $total } lecciones aprobadas
sections-rule = Para aprobar: 3 estrellas o más en cada lección, y 5 estrellas en la lección 30.
sections-best = Mejor: { $value }
sections-lesson-title = Sección de { $section } PPM · Lección { $lesson }
sections-goal = Meta: { $value } PPM
sections-hint = { $done }/{ $total } · Esc: reintentar · Ctrl+A: colores · Backspace corrige
sections-retry = Reintentar (Esc)
sections-score = Puntaje
sections-record = ¡Nuevo récord!
sections-passed = ¡Aprobó la sección de { $section } PPM!
sections-completed = ¡Completó la Progresión!
sections-need-pass = Necesita 3 estrellas para abrir la lección siguiente.
sections-need-goal = Necesita 5 estrellas en la lección 30 para aprobar la sección.
sections-goal-speed = Para { $stars } ★: { $speed } PPM (te faltaron { $missing }).
sections-goal-accuracy = Para { $stars } ★: { $accuracy } % de precisión (lograste { $value } %).
sections-goal-both = Para { $stars } ★: { $speed } PPM y { $accuracy } % de precisión.
sections-end-next = Intro: lección siguiente · Esc: reintentar
sections-end-retry = Intro o Esc: reintentar
sections-end-completed = Intro: volver a las lecciones · Esc: reintentar
