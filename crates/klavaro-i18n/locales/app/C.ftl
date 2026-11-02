# SPDX-License-Identifier: GPL-3.0-or-later
# Textos propios de klavaro-rs, en inglés (idioma de respaldo). Cada traducción en este
# directorio debe tener las mismas claves; las que falten se muestran en inglés.
# Los textos que vienen del Klavaro original están en ../klavaro/ y se generan con
# tools/po2ftl.py: una clave definida acá reemplaza a la del original.

app-name = klavaro-rs
menu-interface-language = Interface:
menu-text-language = Texts:
menu-colors-dark = Dark colors
menu-colors-light = Light colors
menu-shortcuts = Shortcuts: 1–4 open a module · P progress · T Top 10 · S settings · Ctrl+A switches colors
tutor-new = New (Esc)
tutor-lesson-n = Lesson { $lesson }
tutor-hint = { $done }/{ $total } · Esc: new exercise · Ctrl+A: colors
tutor-hint-backspace = { $done }/{ $total } · Esc: new exercise · Ctrl+A: colors · Backspace corrects
stats-seconds = { $value } s
stats-percent = { $value } %
stats-wpm-value = { $value } WPM
comment-too-many-errors = :-(
ps-invalid = :-P
notice-imported = Imported { $sessions } sessions and the preferences from the original Klavaro.
notice-error = Your data could not be read or saved: { $error }
error-no-data = No data found in { $dir }
practice-instructions = Copy each line and press Enter at the end of each one.
progress-all-lessons = All lessons
progress-empty = No sessions to chart yet.
progress-sessions = Last { $count } sessions · { $first } – { $last }
progress-goal = Goal: { $value }
ranking-place = #
tutor-special = Weak keys
tutor-special-on = Weak keys ✓
progress-keys = { $count } keys · { $keyboard }
menu-settings = Settings
settings-name = Name for the Top 10:
settings-name-tip = Leave it empty to use the system user name.
settings-paragraphs-full = Whole text
settings-autostart = Open the last module on startup
settings-goals = Goals
settings-goals-tip = Accuracy and fluidity in %, speed in WPM. The comment levels are edited in config.toml.
settings-defaults = Default goals
settings-save = Save
settings-saved = Settings saved.
settings-invalid = Check the fields in red: accuracy and fluidity go from 0 to 100, speed from 0 to 200.
