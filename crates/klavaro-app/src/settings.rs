// SPDX-License-Identifier: GPL-3.0-or-later
//! Formulario de preferencias, independiente de iced.
//!
//! El original sólo permite cambiar las metas editando su archivo de preferencias; acá se
//! editan en pantalla. Los campos numéricos se guardan como texto mientras se escriben y se
//! validan al guardar, con los mismos rangos que [`Config::sanitized`]. Los niveles de cada
//! comentario siguen editándose sólo en `config.toml`.

use klavaro_core::Module;
use klavaro_core::generators::fluidity::MAX_PARAGRAPHS;
use klavaro_core::goals::{Goal, Goals};
use klavaro_store::Config;

/// Campo de una meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoalField {
    /// Precisión (%).
    Accuracy,
    /// Velocidad (PPM).
    Speed,
    /// Fluidez (sólo módulo de Fluidez).
    Fluidity,
}

impl GoalField {
    /// Rango válido.
    fn range(self) -> (f64, f64) {
        match self {
            Self::Accuracy | Self::Fluidity => (0.0, 100.0),
            Self::Speed => (0.0, 200.0),
        }
    }
}

/// Meta de un módulo tal como se está escribiendo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalInput {
    /// Precisión.
    pub(crate) accuracy: String,
    /// Velocidad.
    pub(crate) speed: String,
    /// Fluidez (`None` si el módulo no la usa).
    pub(crate) fluidity: Option<String>,
}

impl GoalInput {
    fn new(goal: Goal) -> Self {
        Self {
            accuracy: number(goal.accuracy),
            speed: number(goal.speed),
            fluidity: goal.fluidity.map(number),
        }
    }

    /// Texto de un campo (`None` si el módulo no lo tiene).
    pub(crate) fn get(&self, field: GoalField) -> Option<&str> {
        match field {
            GoalField::Accuracy => Some(&self.accuracy),
            GoalField::Speed => Some(&self.speed),
            GoalField::Fluidity => self.fluidity.as_deref(),
        }
    }

    fn set(&mut self, field: GoalField, value: String) {
        match field {
            GoalField::Accuracy => self.accuracy = value,
            GoalField::Speed => self.speed = value,
            GoalField::Fluidity => {
                if let Some(f) = &mut self.fluidity {
                    *f = value;
                }
            }
        }
    }
}

/// Estado del formulario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsForm {
    /// Nombre para el Top 10 (vacío = el del usuario del sistema).
    pub(crate) name: String,
    /// Párrafos de Fluidez (0 = texto completo).
    pub(crate) paragraphs: u8,
    /// Abrir directamente el último módulo usado.
    pub(crate) autostart: bool,
    /// Pitido al equivocarse.
    pub(crate) beep: bool,
    /// Metas en el orden de [`Module::ALL`].
    pub(crate) goals: [GoalInput; 4],
}

impl SettingsForm {
    /// Formulario con los valores de la configuración.
    pub(crate) fn new(config: &Config) -> Self {
        let mut form = Self {
            name: config.player_name.clone().unwrap_or_default(),
            paragraphs: config.fluid_paragraphs,
            autostart: config.autostart,
            beep: config.beep,
            goals: Module::ALL.map(|_| GoalInput::new(Goals::default().basic)),
        };
        form.set_goals(&config.goals);
        form
    }

    fn set_goals(&mut self, goals: &Goals) {
        self.goals = Module::ALL.map(|m| GoalInput::new(goals.for_module(m)));
    }

    /// Meta de un módulo.
    pub(crate) fn goal(&self, module: Module) -> &GoalInput {
        &self.goals[index(module)]
    }

    /// Cambia el texto de un campo de meta.
    pub(crate) fn set_goal(&mut self, module: Module, field: GoalField, value: String) {
        self.goals[index(module)].set(field, value);
    }

    /// Suma `delta` a los párrafos de Fluidez, entre 0 y el máximo.
    pub(crate) fn add_paragraphs(&mut self, delta: i32) {
        let max = i32::try_from(MAX_PARAGRAPHS).unwrap_or(10);
        let n = (i32::from(self.paragraphs) + delta).clamp(0, max);
        self.paragraphs = u8::try_from(n).unwrap_or(0);
    }

    /// Vuelve las metas a los valores del original.
    pub(crate) fn default_goals(&mut self) {
        self.set_goals(&Goals::default());
    }

    /// Si el texto de un campo no es válido.
    pub(crate) fn is_invalid(&self, module: Module, field: GoalField) -> bool {
        self.goal(module)
            .get(field)
            .is_some_and(|v| parse(v, field).is_none())
    }

    /// Configuración con los cambios aplicados, o los campos inválidos.
    pub(crate) fn apply(&self, config: &Config) -> Result<Config, Vec<(Module, GoalField)>> {
        let mut out = config.clone();
        let mut invalid = Vec::new();
        for module in Module::ALL {
            let input = self.goal(module);
            let mut read = |field: GoalField, text: &str| {
                parse(text, field).or_else(|| {
                    invalid.push((module, field));
                    None
                })
            };
            let accuracy = read(GoalField::Accuracy, &input.accuracy);
            let speed = read(GoalField::Speed, &input.speed);
            let fluidity = input
                .fluidity
                .as_deref()
                .map(|f| read(GoalField::Fluidity, f));
            let goal = goal_mut(&mut out.goals, module);
            if let Some(v) = accuracy {
                goal.accuracy = v;
            }
            if let Some(v) = speed {
                goal.speed = v;
            }
            if let Some(Some(v)) = fluidity {
                goal.fluidity = Some(v);
            }
        }
        if !invalid.is_empty() {
            return Err(invalid);
        }
        let name = self.name.trim();
        out.player_name = (!name.is_empty()).then(|| name.to_owned());
        out.fluid_paragraphs = self.paragraphs;
        out.autostart = self.autostart;
        out.beep = self.beep;
        Ok(out.sanitized())
    }
}

fn index(module: Module) -> usize {
    Module::ALL.iter().position(|&m| m == module).unwrap_or(0)
}

fn goal_mut(goals: &mut Goals, module: Module) -> &mut Goal {
    match module {
        Module::Basic => &mut goals.basic,
        Module::Adaptability => &mut goals.adaptability,
        Module::Velocity => &mut goals.velocity,
        Module::Fluidity => &mut goals.fluidity,
    }
}

/// Número sin decimales sobrantes (`95`, `97.5`).
fn number(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Lee un número en su rango; acepta coma decimal.
fn parse(text: &str, field: GoalField) -> Option<f64> {
    let v: f64 = text.trim().replace(',', ".").parse().ok()?;
    let (min, max) = field.range();
    (v.is_finite() && (min..=max).contains(&v)).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_from_the_config() {
        let config = Config {
            player_name: Some("Ana".into()),
            fluid_paragraphs: 5,
            ..Config::default()
        };
        let form = SettingsForm::new(&config);
        assert_eq!(form.name, "Ana");
        assert_eq!(form.paragraphs, 5);
        assert_eq!(form.goal(Module::Adaptability).accuracy, "98");
        assert_eq!(form.goal(Module::Fluidity).fluidity.as_deref(), Some("70"));
        assert_eq!(form.goal(Module::Basic).fluidity, None);
        assert_eq!(form.apply(&config), Ok(config));
    }

    #[test]
    fn applies_valid_changes() {
        let config = Config::default();
        let mut form = SettingsForm::new(&config);
        form.name = "  ".into();
        form.autostart = true;
        form.beep = true;
        form.add_paragraphs(100);
        form.set_goal(Module::Velocity, GoalField::Speed, "62,5".into());
        form.set_goal(Module::Fluidity, GoalField::Fluidity, "80".into());
        // Básico no tiene meta de fluidez: se ignora.
        form.set_goal(Module::Basic, GoalField::Fluidity, "10".into());
        let out = form.apply(&config).expect("válido");
        assert_eq!(out.player_name, None);
        assert!(out.autostart);
        assert!(out.beep);
        assert_eq!(out.fluid_paragraphs, 10);
        assert!((out.goals.velocity.speed - 62.5).abs() < 1e-9);
        assert_eq!(out.goals.fluidity.fluidity, Some(80.0));
        assert_eq!(out.goals.basic.fluidity, None);
    }

    #[test]
    fn rejects_out_of_range_or_garbage() {
        let config = Config::default();
        let mut form = SettingsForm::new(&config);
        form.set_goal(Module::Basic, GoalField::Accuracy, "101".into());
        form.set_goal(Module::Velocity, GoalField::Speed, "rápido".into());
        assert!(form.is_invalid(Module::Basic, GoalField::Accuracy));
        assert!(!form.is_invalid(Module::Basic, GoalField::Speed));
        assert_eq!(
            form.apply(&config),
            Err(vec![
                (Module::Basic, GoalField::Accuracy),
                (Module::Velocity, GoalField::Speed),
            ])
        );
        form.default_goals();
        assert!(form.apply(&config).is_ok());
    }

    #[test]
    fn paragraphs_stay_in_range() {
        let mut form = SettingsForm::new(&Config::default());
        form.add_paragraphs(-100);
        assert_eq!(form.paragraphs, 0);
    }
}
