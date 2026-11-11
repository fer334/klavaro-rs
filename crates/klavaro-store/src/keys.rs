// SPDX-License-Identifier: GPL-3.0-or-later
//! Rastreo de errores y tiempos por tecla de cada teclado (tablas `key_errors` y `key_times`),
//! el equivalente de `accuracy.log_<teclado>` y `proficiency.log_<teclado>` del original, y
//! rastreo por par de letras (tabla `pair_times`, ver `PLAN_TRANSICIONES_DEBILES.md`).
//!
//! A diferencia del original, que sólo guarda el promedio de cada tecla, se guardan los
//! últimos tiempos tal cual.

use klavaro_core::pairs::{PairEntry, PairTracker};
use klavaro_core::tracker::{ErrorEntry, KeyTracker, TimeEntry};
use klavaro_core::transitions::TransitionKind;
use rusqlite::params;

use crate::Result;
use crate::history::History;

impl History {
    /// Rastreo guardado de un teclado (vacío si no hay).
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn tracker(&self, keyboard: &str) -> Result<KeyTracker> {
        let mut stmt = self.conn.prepare(
            "SELECT ch, wrong, correct FROM key_errors WHERE keyboard = ?1 ORDER BY pos",
        )?;
        let errors = stmt
            .query_map([keyboard], |row| {
                Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(ch, wrong, correct)| {
                Some(ErrorEntry {
                    ch: ch.chars().next()?,
                    wrong,
                    correct,
                })
            })
            .collect();
        let mut stmt = self
            .conn
            .prepare("SELECT ch, samples FROM key_times WHERE keyboard = ?1 ORDER BY pos")?;
        let times = stmt
            .query_map([keyboard], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(ch, samples)| {
                let samples = samples
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();
                Some(TimeEntry::new(ch.chars().next()?, samples))
            })
            .collect();
        Ok(KeyTracker::from_parts(errors, times))
    }

    /// Reemplaza el rastreo guardado de un teclado.
    ///
    /// # Errors
    ///
    /// Si falla la escritura (no se cambia nada).
    pub fn save_tracker(&mut self, keyboard: &str, tracker: &KeyTracker) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM key_errors WHERE keyboard = ?1", [keyboard])?;
        tx.execute("DELETE FROM key_times WHERE keyboard = ?1", [keyboard])?;
        for (pos, e) in tracker.errors().iter().enumerate() {
            tx.execute(
                "INSERT INTO key_errors (keyboard, pos, ch, wrong, correct)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    keyboard,
                    i64::try_from(pos).unwrap_or(i64::MAX),
                    e.ch.to_string(),
                    e.wrong,
                    e.correct
                ],
            )?;
        }
        for (pos, t) in tracker.times().iter().enumerate() {
            let samples: Vec<String> = t.samples().iter().map(f64::to_string).collect();
            tx.execute(
                "INSERT INTO key_times (keyboard, pos, ch, samples) VALUES (?1, ?2, ?3, ?4)",
                params![
                    keyboard,
                    i64::try_from(pos).unwrap_or(i64::MAX),
                    t.ch.to_string(),
                    samples.join(" ")
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Rastreo por par de letras guardado de un teclado (vacío si no hay).
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn pairs(&self, keyboard: &str) -> Result<PairTracker> {
        let mut stmt = self.conn.prepare(
            "SELECT pair, kind, samples, typed, wrong FROM pair_times
             WHERE keyboard = ?1 ORDER BY pair",
        )?;
        let entries = stmt
            .query_map([keyboard], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(pair, kind, samples, typed, wrong)| {
                let mut letters = pair.chars();
                let (from, to) = (letters.next()?, letters.next()?);
                let samples = samples
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();
                Some(PairEntry::new(
                    from,
                    to,
                    TransitionKind::from_code(&kind),
                    samples,
                    typed,
                    wrong,
                ))
            });
        Ok(PairTracker::from_entries(entries))
    }

    /// Reemplaza el rastreo por par guardado de un teclado.
    ///
    /// # Errors
    ///
    /// Si falla la escritura (no se cambia nada).
    pub fn save_pairs(&mut self, keyboard: &str, pairs: &PairTracker) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM pair_times WHERE keyboard = ?1", [keyboard])?;
        for e in pairs.entries() {
            let samples: Vec<String> = e.samples().iter().map(f64::to_string).collect();
            tx.execute(
                "INSERT INTO pair_times (keyboard, pair, kind, samples, typed, wrong)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    keyboard,
                    String::from_iter([e.from, e.to]),
                    e.kind.code(),
                    samples.join(" "),
                    e.typed,
                    e.wrong
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn pairs_round_trip_per_keyboard() {
        let mut h = History::in_memory().expect("base");
        let mut pairs = PairTracker::new();
        for iki in [140, 155, 1200] {
            let iki = Some(Duration::from_millis(iki));
            pairs.record('e', 'r', TransitionKind::SameHand, iki);
        }
        pairs.record_error('t', 'h', TransitionKind::OtherHand);
        pairs.record('ñ', 'o', TransitionKind::SameFinger, None);
        h.save_pairs("qwerty_es", &pairs).expect("guardar");
        assert_eq!(h.pairs("qwerty_es").expect("leer"), pairs);
        assert_eq!(h.pairs("dvorak").expect("leer"), PairTracker::new());

        // Guardar de nuevo reemplaza, no duplica.
        h.save_pairs("qwerty_es", &pairs).expect("guardar otra vez");
        assert_eq!(h.pairs("qwerty_es").expect("leer"), pairs);
        h.save_pairs("qwerty_es", &PairTracker::new())
            .expect("vaciar");
        assert!(h.pairs("qwerty_es").expect("leer").is_empty());
    }

    #[test]
    fn tracker_round_trip_per_keyboard() {
        let mut h = History::in_memory().expect("base");
        let tracker = KeyTracker::from_parts(
            vec![
                ErrorEntry {
                    ch: 'ñ',
                    wrong: 4,
                    correct: 2,
                },
                ErrorEntry {
                    ch: 'q',
                    wrong: 1,
                    correct: 9,
                },
            ],
            vec![TimeEntry::new('k', vec![0.25, 0.5])],
        );
        h.save_tracker("qwerty_es", &tracker).expect("guardar");
        assert_eq!(h.tracker("qwerty_es").expect("leer"), tracker);
        assert_eq!(h.tracker("dvorak").expect("leer"), KeyTracker::new());

        h.save_tracker("qwerty_es", &KeyTracker::new())
            .expect("vaciar");
        assert_eq!(h.tracker("qwerty_es").expect("leer"), KeyTracker::new());
    }
}
