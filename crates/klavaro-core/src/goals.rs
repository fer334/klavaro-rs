// SPDX-License-Identifier: GPL-3.0-or-later
//! Metas, niveles y el comentario que se muestra al final de cada ejercicio.
//!
//! Los valores por defecto son los de Klavaro 3.14 (`tutor.c`). El texto de cada comentario
//! no vive aquí: [`Comment`] es una clave con parámetros que la capa de idioma traduce.

use crate::Module;
use crate::metrics::Stats;

/// Meta de un módulo: el nivel a partir del cual se sugiere avanzar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Goal {
    /// Precisión mínima en %.
    pub accuracy: f64,
    /// Velocidad mínima en PPM.
    pub speed: f64,
    /// Fluidez mínima (sólo Fluidez).
    pub fluidity: Option<f64>,
}

/// Metas y niveles de todos los módulos. Editables desde la configuración.
#[derive(Debug, Clone, PartialEq)]
pub struct Goals {
    /// Curso básico.
    pub basic: Goal,
    /// Adaptabilidad.
    pub adaptability: Goal,
    /// Velocidad.
    pub velocity: Goal,
    /// Fluidez.
    pub fluidity: Goal,
    /// Niveles de precisión de Adaptabilidad: aprendiendo, mejorando, llegando.
    pub adapt_levels: [f64; 3],
    /// Niveles de velocidad en PPM: gateando, caminando a pasos, caminando, trotando,
    /// corriendo, profesional, corredor, volando.
    pub velo_levels: [f64; 8],
    /// Niveles de Fluidez: fluidez "tropezando" y velocidad "volando".
    pub fluid_levels: [f64; 2],
}

impl Default for Goals {
    fn default() -> Self {
        Self {
            basic: Goal {
                accuracy: 95.0,
                speed: 10.0,
                fluidity: None,
            },
            adaptability: Goal {
                accuracy: 98.0,
                speed: 10.0,
                fluidity: None,
            },
            velocity: Goal {
                accuracy: 95.0,
                speed: 50.0,
                fluidity: None,
            },
            fluidity: Goal {
                accuracy: 97.0,
                speed: 50.0,
                fluidity: Some(70.0),
            },
            adapt_levels: [50.0, 90.0, 95.0],
            velo_levels: [10.0, 20.0, 30.0, 40.0, 60.0, 70.0, 80.0, 90.0],
            fluid_levels: [60.0, 90.0],
        }
    }
}

impl Goals {
    /// Meta de un módulo.
    pub fn for_module(&self, module: Module) -> Goal {
        match module {
            Module::Basic => self.basic,
            Module::Adaptability => self.adaptability,
            Module::Velocity => self.velocity,
            Module::Fluidity => self.fluidity,
        }
    }

    /// Si un resultado alcanza la meta del módulo en cada métrica (para colorear).
    pub fn evaluate(&self, module: Module, stats: &Stats) -> GoalCheck {
        let goal = self.for_module(module);
        GoalCheck {
            accuracy: stats.accuracy >= goal.accuracy,
            speed: stats.wpm >= goal.speed,
            fluidity: goal.fluidity.map(|f| stats.fluidity >= f),
        }
    }

    /// Comentario de fin de ejercicio, según las reglas de cada módulo del original.
    pub fn comment(&self, module: Module, stats: &Stats, ctx: CommentContext) -> Comment {
        match module {
            Module::Basic => self.basic_comment(stats, ctx),
            Module::Adaptability => self.adapt_comment(stats),
            Module::Velocity => self.velo_comment(stats),
            Module::Fluidity => self.fluid_comment(stats),
        }
    }

    fn basic_comment(&self, stats: &Stats, ctx: CommentContext) -> Comment {
        if stats.accuracy < self.basic.accuracy {
            Comment::TooManyErrors
        } else if ctx.course_completed {
            Comment::BasicCourseCompleted
        } else {
            Comment::BasicNextLesson
        }
    }

    fn adapt_comment(&self, stats: &Stats) -> Comment {
        let [learning, improving, reaching] = self.adapt_levels;
        let acc = stats.accuracy;
        if acc < learning {
            Comment::TooManyErrors
        } else if acc < improving {
            Comment::AdaptTryAgain { target: improving }
        } else if acc < reaching {
            Comment::AdaptImproving { target: reaching }
        } else if acc < self.adaptability.accuracy {
            Comment::AdaptAlmostThere {
                target: self.adaptability.accuracy,
            }
        } else {
            Comment::AdaptDone {
                target: self.adaptability.accuracy,
            }
        }
    }

    /// Diez escalones como `velo_comment` de `velocity.c`: cuatro niveles hasta la meta y
    /// cuatro más allá.
    fn velo_comment(&self, stats: &Stats) -> Comment {
        if stats.accuracy < self.velocity.accuracy {
            return Comment::TooManyErrors;
        }
        let v = stats.wpm;
        let [l0, l1, l2, l3, l4, l5, l6, l7] = self.velo_levels;
        let goal = self.velocity.speed;
        if v < l0 {
            Comment::VeloBeginning
        } else if v < l1 {
            Comment::VeloHighway { target: l1 }
        } else if v < l2 {
            Comment::VeloFaster { target: l2 }
        } else if v < l3 {
            Comment::VeloRunning { target: l3 }
        } else if v < goal {
            Comment::VeloAlmostThere { target: goal }
        } else if v < l4 {
            Comment::VeloEnough
        } else if v < l5 {
            Comment::VeloProfessional { target: l5 }
        } else if v < l6 {
            Comment::VeloSpeedRacer { target: l6 }
        } else if v < l7 {
            Comment::VeloKungFu { target: l7 }
        } else {
            Comment::VeloMaster
        }
    }

    fn fluid_comment(&self, stats: &Stats) -> Comment {
        let goal = self.fluidity;
        let fluid_goal = goal.fluidity.unwrap_or(70.0);
        if stats.accuracy < goal.accuracy {
            Comment::TooManyErrors
        } else if stats.wpm < goal.speed {
            Comment::FluidTooSlow { target: goal.speed }
        } else if stats.fluidity < self.fluid_levels[0] {
            Comment::FluidIrregular {
                target: self.fluid_levels[0],
            }
        } else if stats.fluidity < fluid_goal {
            Comment::FluidAlmostThere { target: fluid_goal }
        } else if stats.wpm < self.fluid_levels[1] {
            Comment::FluidCongratulations
        } else {
            Comment::FluidBeyondExpectations
        }
    }
}

/// Resultado de comparar un ejercicio con la meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoalCheck {
    /// Precisión alcanzada.
    pub accuracy: bool,
    /// Velocidad alcanzada.
    pub speed: bool,
    /// Fluidez alcanzada (sólo módulo Fluidez).
    pub fluidity: Option<bool>,
}

/// Datos extra que algunos comentarios necesitan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommentContext {
    /// En Básico: se acaba de aprobar la última lección fija (43).
    pub course_completed: bool,
}

/// Comentario de fin de ejercicio. Cada variante es una clave de traducción.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comment {
    /// Precisión por debajo de la meta (la cara triste del original).
    TooManyErrors,
    /// Básico aprobado: pasar a la siguiente lección.
    BasicNextLesson,
    /// Básico: se completaron las 43 lecciones.
    BasicCourseCompleted,
    /// Adaptabilidad: intentar de nuevo hasta `target` %.
    AdaptTryAgain {
        /// Precisión objetivo.
        target: f64,
    },
    /// Adaptabilidad: va bien, llegar a `target` %.
    AdaptImproving {
        /// Precisión objetivo.
        target: f64,
    },
    /// Adaptabilidad: casi; probar unas veces más o cambiar de ejercicio.
    AdaptAlmostThere {
        /// Precisión objetivo.
        target: f64,
    },
    /// Adaptabilidad completa (precisión por encima de `target` %): pasar a Velocidad.
    AdaptDone {
        /// Meta de precisión superada.
        target: f64,
    },
    /// Velocidad: recién empieza; paciencia y práctica diaria.
    VeloBeginning,
    /// Velocidad: lejos de la autopista, llegar a `target` PPM.
    VeloHighway {
        /// Velocidad objetivo.
        target: f64,
    },
    /// Velocidad: bien, pero más rápido; llegar a `target` PPM.
    VeloFaster {
        /// Velocidad objetivo.
        target: f64,
    },
    /// Velocidad: empezar a correr hasta `target` PPM.
    VeloRunning {
        /// Velocidad objetivo.
        target: f64,
    },
    /// Velocidad: casi en la meta de `target` PPM.
    VeloAlmostThere {
        /// Velocidad objetivo (la meta).
        target: f64,
    },
    /// Meta de velocidad cumplida: pasar a Fluidez.
    VeloEnough,
    /// Velocidad de profesional; ¿llegará a `target` PPM?
    VeloProfessional {
        /// Siguiente nivel.
        target: f64,
    },
    /// "Speed Racer"; ¿llegará a `target` PPM?
    VeloSpeedRacer {
        /// Siguiente nivel.
        target: f64,
    },
    /// Maestría de kung-fu; ¿volará a `target` PPM?
    VeloKungFu {
        /// Último nivel.
        target: f64,
    },
    /// Más allá del último nivel ("Dvorak master").
    VeloMaster,
    /// Fluidez: preciso pero lento.
    FluidTooSlow {
        /// Velocidad objetivo.
        target: f64,
    },
    /// Fluidez: ritmo irregular.
    FluidIrregular {
        /// Fluidez objetivo.
        target: f64,
    },
    /// Fluidez: casi en la meta.
    FluidAlmostThere {
        /// Fluidez objetivo.
        target: f64,
    },
    /// Fluidez cumplida.
    FluidCongratulations,
    /// Más allá de lo esperado.
    FluidBeyondExpectations,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn stats(accuracy: f64, wpm: f64, fluidity: f64) -> Stats {
        Stats {
            touches: 100,
            errors: 0,
            elapsed: Duration::from_secs(60),
            accuracy,
            wpm,
            fluidity,
        }
    }

    #[test]
    fn basic_comments() {
        let g = Goals::default();
        let ctx = CommentContext::default();
        assert_eq!(
            g.comment(Module::Basic, &stats(90.0, 20.0, 50.0), ctx),
            Comment::TooManyErrors
        );
        assert_eq!(
            g.comment(Module::Basic, &stats(96.0, 20.0, 50.0), ctx),
            Comment::BasicNextLesson
        );
        let done = CommentContext {
            course_completed: true,
        };
        assert_eq!(
            g.comment(Module::Basic, &stats(96.0, 20.0, 50.0), done),
            Comment::BasicCourseCompleted
        );
    }

    #[test]
    fn adapt_comment_levels() {
        let g = Goals::default();
        let c = |acc| {
            g.comment(
                Module::Adaptability,
                &stats(acc, 20.0, 50.0),
                CommentContext::default(),
            )
        };
        assert_eq!(c(40.0), Comment::TooManyErrors);
        assert_eq!(c(80.0), Comment::AdaptTryAgain { target: 90.0 });
        assert_eq!(c(92.0), Comment::AdaptImproving { target: 95.0 });
        assert_eq!(c(96.0), Comment::AdaptAlmostThere { target: 98.0 });
        assert_eq!(c(99.0), Comment::AdaptDone { target: 98.0 });
    }

    #[test]
    fn velo_comment_levels() {
        let g = Goals::default();
        let c = |wpm| {
            g.comment(
                Module::Velocity,
                &stats(99.0, wpm, 50.0),
                CommentContext::default(),
            )
        };
        assert_eq!(c(5.0), Comment::VeloBeginning);
        assert_eq!(c(15.0), Comment::VeloHighway { target: 20.0 });
        assert_eq!(c(25.0), Comment::VeloFaster { target: 30.0 });
        assert_eq!(c(35.0), Comment::VeloRunning { target: 40.0 });
        assert_eq!(c(45.0), Comment::VeloAlmostThere { target: 50.0 });
        assert_eq!(c(55.0), Comment::VeloEnough);
        assert_eq!(c(65.0), Comment::VeloProfessional { target: 70.0 });
        assert_eq!(c(75.0), Comment::VeloSpeedRacer { target: 80.0 });
        assert_eq!(c(85.0), Comment::VeloKungFu { target: 90.0 });
        assert_eq!(c(95.0), Comment::VeloMaster);
        let low = g.comment(
            Module::Velocity,
            &stats(90.0, 95.0, 50.0),
            CommentContext::default(),
        );
        assert_eq!(low, Comment::TooManyErrors);
    }

    #[test]
    fn fluid_comment_levels() {
        let g = Goals::default();
        let c = |a, w, f| g.comment(Module::Fluidity, &stats(a, w, f), CommentContext::default());
        assert_eq!(c(90.0, 60.0, 80.0), Comment::TooManyErrors);
        assert_eq!(c(99.0, 40.0, 80.0), Comment::FluidTooSlow { target: 50.0 });
        assert_eq!(
            c(99.0, 60.0, 50.0),
            Comment::FluidIrregular { target: 60.0 }
        );
        assert_eq!(
            c(99.0, 60.0, 65.0),
            Comment::FluidAlmostThere { target: 70.0 }
        );
        assert_eq!(c(99.0, 60.0, 80.0), Comment::FluidCongratulations);
        assert_eq!(c(99.0, 95.0, 80.0), Comment::FluidBeyondExpectations);
    }

    #[test]
    fn goal_check() {
        let g = Goals::default();
        let check = g.evaluate(Module::Fluidity, &stats(98.0, 40.0, 75.0));
        assert_eq!(
            check,
            GoalCheck {
                accuracy: true,
                speed: false,
                fluidity: Some(true)
            }
        );
    }
}
