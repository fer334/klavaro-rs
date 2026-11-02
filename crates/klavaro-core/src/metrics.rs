// SPDX-License-Identifier: GPL-3.0-or-later
//! Métricas de un ejercicio, con las fórmulas de Klavaro 3.14 (`tutor.c` y `top10.c`).

use std::time::Duration;

/// Valor mínimo de fluidez que informa el original.
pub const MIN_FLUIDITY: f64 = 2.0;

/// Resultado de un ejercicio terminado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// Pulsaciones evaluadas.
    pub touches: u32,
    /// Errores.
    pub errors: u32,
    /// Tiempo desde la primera tecla hasta la última.
    pub elapsed: Duration,
    /// Precisión en %, `100 × (1 − errores / pulsaciones)`. Puede ser negativa en Fluidez.
    pub accuracy: f64,
    /// Velocidad en palabras (de 5 caracteres) por minuto: `12 × (pulsaciones − errores) / s`.
    pub wpm: f64,
    /// Regularidad del ritmo, de 2 a 100.
    pub fluidity: f64,
}

impl Stats {
    /// Calcula las métricas a partir de los conteos y de los intervalos entre aciertos.
    pub fn compute(touches: u32, errors: u32, elapsed: Duration, intervals: &[Duration]) -> Self {
        Self {
            touches,
            errors,
            elapsed,
            accuracy: accuracy(touches, errors),
            wpm: words_per_minute(touches, errors, elapsed),
            fluidity: fluidity(intervals),
        }
    }

    /// Caracteres correctos por segundo.
    pub fn chars_per_second(&self) -> f64 {
        self.wpm / 12.0
    }

    /// Puntaje del Top 10 para este resultado.
    pub fn score(&self) -> f64 {
        score(self.wpm, self.accuracy, self.fluidity)
    }
}

/// Precisión en %. Con 0 pulsaciones devuelve 0.
pub fn accuracy(touches: u32, errors: u32) -> f64 {
    if touches == 0 {
        return 0.0;
    }
    100.0 * (1.0 - f64::from(errors) / f64::from(touches))
}

/// Palabras por minuto. Con tiempo 0 devuelve 0.
pub fn words_per_minute(touches: u32, errors: u32, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    let correct = f64::from(touches.saturating_sub(errors));
    12.0 * correct / secs
}

/// Fluidez: `100 × (1 − σ(x) / x̄)` con `xᵢ = √(1/Δtᵢ)`, igual que `tutor_calc_stats` de
/// Klavaro 3.14.
///
/// `intervals` tiene un valor por acierto: el tiempo desde la tecla anterior (el primero es
/// el del primer toque, ~0). Se descartan los dos primeros. El promedio divide por la
/// cantidad de muestras y la desviación por `n − 1` (como mínimo 1). Nunca baja de
/// [`MIN_FLUIDITY`]; sin muestras suficientes da 100, como el original.
pub fn fluidity(intervals: &[Duration]) -> f64 {
    let samples: Vec<f64> = intervals
        .iter()
        .skip(2)
        .map(|dt| {
            let t = dt.as_secs_f64();
            let t = if t <= 0.0 { 1e-8 } else { t };
            (1.0 / t).sqrt()
        })
        .collect();
    let n = samples.len() as f64;
    let mut mean = samples.iter().sum::<f64>() / n.max(1.0);
    if mean <= 0.0 {
        mean = 1e-9;
    }
    let squares = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
    let deviation = (squares / (n - 1.0).max(1.0)).sqrt();
    (100.0 * (1.0 - deviation / mean)).max(MIN_FLUIDITY)
}

/// Puntaje del Top 10: `(6 × PPM/1,2 + 3 × precisión + 1 × fluidez) / 100`.
pub fn score(wpm: f64, accuracy: f64, fluidity: f64) -> f64 {
    (6.0 * wpm / 1.2 + 3.0 * accuracy + fluidity) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    #[test]
    fn accuracy_formula() {
        assert!((accuracy(200, 10) - 95.0).abs() < 1e-9);
        assert!((accuracy(0, 0)).abs() < 1e-9);
        // En Fluidez los errores corregidos pueden superar las pulsaciones.
        assert!(accuracy(10, 12) < 0.0);
    }

    #[test]
    fn wpm_formula_matches_original() {
        // 300 pulsaciones correctas en 60 s = 5 caracteres/s = 60 PPM.
        let w = words_per_minute(300, 0, Duration::from_secs(60));
        assert!((w - 60.0).abs() < 1e-9);
        assert!((words_per_minute(10, 0, Duration::ZERO)).abs() < 1e-9);
        assert!((words_per_minute(5, 9, Duration::from_secs(1))).abs() < 1e-9);
    }

    #[test]
    fn perfect_rhythm_is_100() {
        let ivs = vec![ms(200); 50];
        assert!((fluidity(&ivs) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn irregular_rhythm_is_lower() {
        let regular = vec![ms(200); 50];
        let irregular: Vec<_> = (0..50)
            .map(|i| ms(if i % 2 == 0 { 100 } else { 600 }))
            .collect();
        assert!(fluidity(&irregular) < fluidity(&regular));
        assert!(fluidity(&irregular) >= MIN_FLUIDITY);
    }

    #[test]
    fn too_few_samples_behave_like_original() {
        // Sin muestras útiles la desviación es 0: el original informa 100.
        assert!((fluidity(&[ms(0), ms(100)]) - 100.0).abs() < 1e-9);
        assert!((fluidity(&[ms(0), ms(100), ms(300)]) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn matches_hand_computed_value() {
        // Muestras 0,25 s y 1 s → x = 2 y 1; media 1,5; σ = √(0,5 / 1) ≈ 0,7071.
        let ivs = [ms(0), ms(0), ms(250), ms(1_000)];
        let expected = 100.0 * (1.0 - 0.5f64.sqrt() / 1.5);
        assert!((fluidity(&ivs) - expected).abs() < 1e-9);
    }

    #[test]
    fn very_irregular_rhythm_is_floored() {
        let ivs = [ms(0), ms(0), ms(1), ms(3_000), ms(1), ms(3_000)];
        assert!((fluidity(&ivs) - MIN_FLUIDITY).abs() < 1e-9);
    }

    #[test]
    fn score_formula() {
        // 60 PPM, 100 %, 100 de fluidez → (300 + 300 + 100) / 100 = 7.
        assert!((score(60.0, 100.0, 100.0) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn stats_compute_and_cps() {
        let s = Stats::compute(120, 0, Duration::from_secs(24), &[ms(200); 10]);
        assert!((s.wpm - 60.0).abs() < 1e-9);
        assert!((s.chars_per_second() - 5.0).abs() < 1e-9);
        assert!((s.accuracy - 100.0).abs() < 1e-9);
    }
}
