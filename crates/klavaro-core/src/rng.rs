// SPDX-License-Identifier: GPL-3.0-or-later
//! Azar inyectable.
//!
//! Los generadores reciben un `&mut impl RandomSource`. En producción se usa [`SplitMix64`]
//! con una semilla tomada del reloj; en pruebas, una semilla fija para obtener siempre el
//! mismo texto.

/// Fuente de números pseudoaleatorios.
pub trait RandomSource {
    /// Devuelve 64 bits pseudoaleatorios.
    fn next_u64(&mut self) -> u64;

    /// Entero uniforme en `0..n`. Devuelve 0 si `n == 0`.
    fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        let n = n as u64;
        // Rechazo para evitar el sesgo del módulo.
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let x = self.next_u64();
            if x < zone {
                // `x % n < n <= usize::MAX`, la conversión no pierde información.
                return usize::try_from(x % n).unwrap_or(0);
            }
        }
    }

    /// `true` con probabilidad `1 / n` (como el `rand() % n == 0` del original).
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    /// Elemento al azar de un slice.
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.below(items.len()))
        }
    }
}

/// Generador `SplitMix64`: pequeño, rápido y de buena calidad estadística para juegos y
/// ejercicios (no es criptográfico).
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Crea un generador con una semilla dada.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl RandomSource for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_stays_in_range_and_covers_all_values() {
        let mut rng = SplitMix64::new(7);
        let mut seen = [false; 6];
        for _ in 0..1_000 {
            let v = rng.below(6);
            assert!(v < 6);
            seen[v] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn below_zero_and_one_are_zero() {
        let mut rng = SplitMix64::new(1);
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(1), 0);
    }

    #[test]
    fn pick_from_empty_is_none() {
        let mut rng = SplitMix64::new(1);
        let empty: [u8; 0] = [];
        assert!(rng.pick(&empty).is_none());
    }
}
