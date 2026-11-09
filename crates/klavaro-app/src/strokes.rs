// SPDX-License-Identifier: GPL-3.0-or-later
//! Registro de cuándo se aprieta y se suelta la tecla de cada letra, para medir transiciones.
//!
//! Sólo observa: no cambia cómo se evalúa el ejercicio. Lo usa la Progresión (ver
//! `PLAN_TRANSICIONES.md`). Si falta un dato (una letra del método de entrada, una tecla que
//! nunca se soltó) queda en `None` y la transición se trata como «sin medir».

use std::collections::HashMap;
use std::time::Duration;

use klavaro_core::session::CharState;

/// Identidad de la tecla física, independiente del framework de interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct KeyId(pub(crate) u64);

/// Datos de una pulsación que acompañan a la acción traducida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stroke {
    /// Instante en que llegó el evento.
    pub(crate) at: Duration,
    /// Tecla física (`None` si el texto vino del método de entrada).
    pub(crate) key: Option<KeyId>,
    /// Autorrepetición por mantener la tecla.
    pub(crate) repeat: bool,
}

impl Stroke {
    /// Pulsación sin tecla física conocida.
    pub(crate) fn bare(at: Duration) -> Self {
        Self {
            at,
            key: None,
            repeat: false,
        }
    }
}

/// Tiempos de la última pulsación que escribió una letra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timing {
    /// Cuándo bajó la tecla.
    pub(crate) down: Duration,
    /// Cuándo se soltó, si ya se soltó y se pudo unir con su pulsación.
    pub(crate) up: Option<Duration>,
    /// Tecla física, si se conoce.
    pub(crate) key: Option<KeyId>,
}

/// Tiempos por posición del texto objetivo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StrokeLog {
    timings: Vec<Option<Timing>>,
    /// Teclas apretadas que escribieron una letra y todavía no se soltaron.
    held: HashMap<KeyId, usize>,
    /// Letras escritas por la autorrepetición del sistema (no tienen tiempos propios).
    repeated: Vec<bool>,
    /// Ya se imprimió la tabla de depuración.
    reported: bool,
}

impl StrokeLog {
    /// Registro vacío para un texto de `len` caracteres.
    pub(crate) fn new(len: usize) -> Self {
        Self {
            timings: vec![None; len],
            held: HashMap::new(),
            repeated: vec![false; len],
            reported: false,
        }
    }

    /// `true` la primera vez que se llama con todas las teclas sueltas: es el momento de
    /// imprimir la tabla, con todos los tiempos completos.
    pub(crate) fn take_report(&mut self) -> bool {
        if self.reported || !self.held.is_empty() {
            return false;
        }
        self.reported = true;
        true
    }

    /// Tiempos alineados con el texto objetivo.
    // Lo usan las pruebas y el cálculo de transiciones (Fase 2 de PLAN_TRANSICIONES.md).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn timings(&self) -> &[Option<Timing>] {
        &self.timings
    }

    /// La pulsación `stroke` escribió la letra `pos`. Si se reescribe, vale la última. Una
    /// autorrepetición deja la letra sin tiempos.
    pub(crate) fn down(&mut self, pos: usize, stroke: Stroke) {
        let Some(slot) = self.timings.get_mut(pos) else {
            return;
        };
        if stroke.repeat {
            // La autorrepetición escribe una letra, pero la tecla bajó antes: no hay tiempos.
            *slot = None;
            self.repeated[pos] = true;
            return;
        }
        *slot = Some(Timing {
            down: stroke.at,
            up: None,
            key: stroke.key,
        });
        self.repeated[pos] = false;
        if let Some(key) = stroke.key {
            self.held.insert(key, pos);
        }
    }

    /// Se soltó `key` en el instante `at`.
    pub(crate) fn up(&mut self, key: KeyId, at: Duration) {
        let Some(pos) = self.held.remove(&key) else {
            return;
        };
        if let Some(Some(t)) = self.timings.get_mut(pos)
            && t.key == Some(key)
            && t.up.is_none()
        {
            t.up = Some(at.max(t.down));
        }
    }

    /// Tabla legible de los tiempos, para depurar.
    pub(crate) fn dump(&self, target: &[char], states: &[CharState]) -> String {
        use std::fmt::Write as _;
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut out =
            String::from("pos\tletra\testado\tdown_ms\tup_ms\tretención\tIKI\trollover\n");
        let mut prev: Option<Timing> = None;
        for (i, (c, t)) in target.iter().zip(&self.timings).enumerate() {
            let shown = match c {
                '\n' => "¶".to_owned(),
                ' ' => "␣".to_owned(),
                c => c.to_string(),
            };
            let state = match states.get(i) {
                Some(CharState::Correct) => "bien",
                Some(CharState::Wrong) => "mal",
                Some(CharState::Retouched) => "corregida",
                Some(CharState::Untouched) | None => "-",
            };
            let Some(t) = t else {
                let why = if self.repeated[i] {
                    "autorrepetición"
                } else {
                    "-"
                };
                let _ = writeln!(out, "{i}\t{shown}\t{state}\t{why}");
                prev = None;
                continue;
            };
            let up = t.up.map_or("-".to_owned(), |u| format!("{:.1}", ms(u)));
            let hold = t.up.map_or("-".to_owned(), |u| {
                format!("{:.1}", ms(u.saturating_sub(t.down)))
            });
            let iki = prev.map_or("-".to_owned(), |p| {
                format!("{:.1}", ms(t.down.saturating_sub(p.down)))
            });
            let rollover = match prev.and_then(|p| p.up) {
                Some(pu) if pu > t.down => "sí",
                Some(_) => "no",
                None => "-",
            };
            let _ = writeln!(
                out,
                "{i}\t{shown}\t{state}\t{:.1}\t{up}\t{hold}\t{iki}\t{rollover}",
                ms(t.down)
            );
            prev = Some(*t);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn stroke(at: u64, key: u64) -> Stroke {
        Stroke {
            at: ms(at),
            key: Some(KeyId(key)),
            repeat: false,
        }
    }

    #[test]
    fn joins_each_release_with_its_press() {
        let mut log = StrokeLog::new(2);
        log.down(0, stroke(0, 1));
        log.down(1, stroke(80, 2));
        // Rollover: la primera se suelta después de bajar la segunda.
        log.up(KeyId(1), ms(100));
        log.up(KeyId(2), ms(170));
        let t = log.timings();
        assert_eq!(t[0].and_then(|t| t.up), Some(ms(100)));
        assert_eq!(t[1].and_then(|t| t.up), Some(ms(170)));
    }

    #[test]
    fn ignores_releases_of_keys_that_wrote_nothing() {
        let mut log = StrokeLog::new(1);
        // Shift se suelta sin haber escrito una letra.
        log.up(KeyId(9), ms(10));
        log.down(0, stroke(20, 1));
        log.up(KeyId(9), ms(30));
        assert_eq!(log.timings()[0].and_then(|t| t.up), None);
        log.up(KeyId(1), ms(90));
        assert_eq!(log.timings()[0].and_then(|t| t.up), Some(ms(90)));
    }

    #[test]
    fn a_rewritten_letter_keeps_only_the_last_press() {
        let mut log = StrokeLog::new(1);
        log.down(0, stroke(0, 1));
        log.down(0, stroke(500, 2));
        // La tecla vieja ya no es la de esta letra.
        log.up(KeyId(1), ms(510));
        assert_eq!(log.timings()[0].and_then(|t| t.up), None);
        log.up(KeyId(2), ms(560));
        assert_eq!(log.timings()[0].and_then(|t| t.up), Some(ms(560)));
    }

    #[test]
    fn a_repeated_letter_has_no_timings_and_is_marked() {
        let mut log = StrokeLog::new(2);
        log.down(0, stroke(0, 1));
        log.down(
            1,
            Stroke {
                repeat: true,
                ..stroke(300, 1)
            },
        );
        assert_eq!(log.timings()[1], None);
        let states = [CharState::Correct, CharState::Wrong];
        assert!(
            log.dump(&['h', 't'], &states)
                .contains("1\tt\tmal\tautorrepetición")
        );
        // Reescribirla con una pulsación real borra la marca.
        log.down(1, stroke(600, 2));
        assert!(!log.dump(&['h', 't'], &states).contains("autorrepetición"));
    }

    #[test]
    fn letters_without_a_physical_key_have_no_release() {
        let mut log = StrokeLog::new(2);
        log.down(0, Stroke::bare(ms(0)));
        log.down(5, stroke(0, 1)); // Fuera del texto: se ignora.
        assert_eq!(log.timings()[0].map(|t| t.key), Some(None));
        let states = [CharState::Correct, CharState::Untouched];
        assert!(
            log.dump(&['a', 'b'], &states)
                .contains("0\ta\tbien\t0.0\t-")
        );
    }
}
