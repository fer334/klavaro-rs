// SPDX-License-Identifier: GPL-3.0-or-later
//! Pruebas "golden": todos los datos de Klavaro 3.14 copiados en `assets/` se leen sin error
//! y alcanzan para generar ejercicios de los cuatro módulos.

// Archivo de pruebas: un fallo de E/S debe detener la prueba con un mensaje claro.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use klavaro_core::generators::{LanguageRules, adaptability, basic, fluidity, velocity};
use klavaro_core::keyboard::Finger;
use klavaro_core::progression;
use klavaro_core::rng::SplitMix64;
use klavaro_data::{catalog, kbd, lessons, texts};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn files_with_ext(ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(assets())
        .expect("carpeta assets/")
        .map(|e| e.expect("entrada").path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    out.sort();
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .expect("nombre")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn every_keyboard_layout_parses() {
    let files = files_with_ext("kbd");
    assert!(
        files.len() >= 70,
        "se esperaban las ~77 distribuciones del original"
    );
    for path in files {
        let layout = kbd::parse(&stem(&path), &read(&path))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            layout.chars().len() >= 20,
            "{} tiene muy pocas teclas",
            path.display()
        );
        // Escribir y volver a leer no cambia nada.
        let again = kbd::parse(layout.name(), &kbd::serialize(&layout)).expect("ida y vuelta");
        assert_eq!(again, layout, "{}", path.display());
    }
}

#[test]
fn basic_course_has_43_usable_lessons_on_every_layout() {
    let masks = lessons::parse_basic_lessons(&read(&assets().join("basic_lessons.txt")))
        .expect("basic_lessons.txt");
    assert_eq!(masks.len(), usize::from(basic::FIXED_LESSONS));
    let rules = LanguageRules::default();
    for path in files_with_ext("kbd") {
        let layout = kbd::parse(&stem(&path), &read(&path)).expect("kbd");
        for (i, mask) in masks.iter().enumerate() {
            let set = mask.charset(&layout);
            if set.len() < 2 {
                // Algunas distribuciones especiales (por ejemplo, de una mano) no tienen
                // teclas en todas las posiciones de las primeras lecciones.
                continue;
            }
            let text = basic::generate(&set, &rules, &mut SplitMix64::new(i as u64))
                .unwrap_or_else(|| panic!("{} lección {}", path.display(), i + 1));
            assert_eq!(text.lines().count(), basic::LINES);
        }
    }
}

#[test]
fn qwerty_lesson_one_is_f_and_j() {
    let masks = lessons::parse_basic_lessons(&read(&assets().join("basic_lessons.txt")))
        .expect("basic_lessons.txt");
    let layout = kbd::parse("qwerty_us", &read(&assets().join("qwerty_us.kbd"))).expect("kbd");
    let mut first = masks[0].charset(&layout);
    first.sort_unstable();
    assert_eq!(first, vec!['f', 'j']);
}

#[test]
fn finger_map_parses() {
    let map = lessons::parse_fingers(&read(&assets().join("fingers_position.txt")))
        .expect("fingers_position.txt");
    let layout = kbd::parse("qwerty_us", &read(&assets().join("qwerty_us.kbd"))).expect("kbd");
    assert_eq!(map.finger_for(&layout, 'f'), Some(Finger::LeftIndex));
    assert_eq!(map.finger_for(&layout, 'j'), Some(Finger::RightIndex));
    assert_eq!(map.finger_for(&layout, 'a'), Some(Finger::LeftPinky));
}

#[test]
fn every_language_has_words_and_paragraphs_that_generate_exercises() {
    let words_files = files_with_ext("words");
    assert!(words_files.len() >= 30);
    for path in words_files {
        let lang = stem(&path);
        let rules = LanguageRules::new(lang.clone());
        let words = texts::parse_words(&read(&path));
        assert!(words.len() >= 20, "{lang}: pocas palabras");
        let text = velocity::generate(&words, &rules, &mut SplitMix64::new(1)).expect("velocidad");
        assert_eq!(text.lines().count(), velocity::PARAGRAPHS, "{lang}");

        let par_path = path.with_extension("paragraphs");
        let paragraphs = texts::parse_paragraphs(&read(&par_path));
        // El tibetano trae sólo 2 párrafos; el resto, muchos más.
        assert!(paragraphs.len() >= 2, "{lang}: faltan párrafos");
        let text =
            fluidity::select(&paragraphs, 3, false, &mut SplitMix64::new(1)).expect("fluidez");
        assert_eq!(text.lines().count(), paragraphs.len().min(3), "{lang}");
    }
}

#[test]
fn adaptability_works_on_every_layout() {
    for path in files_with_ext("kbd") {
        let layout = kbd::parse(&stem(&path), &read(&path)).expect("kbd");
        let text = adaptability::generate(
            &layout,
            &LanguageRules::default(),
            None,
            &mut SplitMix64::new(7),
        );
        assert_eq!(
            text.lines().count(),
            adaptability::PARAGRAPHS,
            "{}",
            path.display()
        );
    }
}

#[test]
fn the_progression_catalog_is_complete_and_attributed() {
    let entries = catalog::parse(&read(&assets().join("C.progression"))).expect("catálogo");
    assert_eq!(entries.len(), 510);
    let mut positions = HashSet::new();
    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    for entry in &entries {
        let slot = entry.slot;
        assert!(positions.insert(slot), "{slot:?} repetida");
        assert!(
            ids.insert(entry.text_id.clone()),
            "{} repetido",
            entry.text_id
        );
        assert!(titles.insert(entry.title.to_lowercase()), "{}", entry.title);
        assert!(!entry.title.is_empty() && entry.revision > 0, "{slot:?}");
        assert!(
            ["en", "simple"].contains(&entry.source.as_str()),
            "{slot:?}"
        );
        assert!(entry.text_id.starts_with(&entry.source), "{slot:?}");
        assert!(
            entry.text.is_ascii() && entry.text.ends_with('.'),
            "{slot:?}"
        );
        let chars = entry.text.chars().count() as f64;
        let target = slot.target_chars();
        assert!(
            chars >= target * 0.85 && chars <= target * 1.15,
            "{slot:?}: {chars} caracteres, se pedían {target}"
        );
    }
    let mean = |section: u16| {
        let of_section: Vec<f64> = entries
            .iter()
            .filter(|e| e.slot.section() == section)
            .map(|e| e.difficulty)
            .collect();
        assert_eq!(of_section.len(), 30);
        of_section.iter().sum::<f64>() / 30.0
    };
    let means: Vec<f64> = progression::sections().map(mean).collect();
    assert!(means.windows(2).all(|w| w[0] < w[1]), "{means:?}");
}

#[test]
fn the_progression_can_be_typed_on_english_keyboards() {
    let entries = catalog::parse(&read(&assets().join("C.progression"))).expect("catálogo");
    for name in [
        "qwerty_us",
        "qwerty_uk",
        "dvorak_us",
        "dvorak_uk",
        "colemak_us",
    ] {
        let path = assets().join(format!("{name}.kbd"));
        let layout = kbd::parse(name, &read(&path)).expect("teclado");
        for entry in &entries {
            let missing: Vec<char> = entry
                .text
                .chars()
                .filter(|&c| c != ' ' && !layout.contains(c))
                .collect();
            assert_eq!(missing, Vec::<char>::new(), "{name} {:?}", entry.slot);
        }
    }
}
