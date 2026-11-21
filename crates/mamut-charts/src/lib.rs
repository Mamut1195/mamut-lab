//! `mamut-charts` — geometría de gráficos técnicos.
//!
//! Este crate **no dibuja**: decide dónde va cada punto, cada marca de eje y cada línea
//! de límite. La interfaz convierte el resultado en primitivas de Rune y el informe en
//! vectores del PDF, de modo que **la pantalla y el papel muestran el mismo gráfico**
//! porque comparten la geometría (ADR-0004).
//!
//! Se prueba sin GPU y sin ventana: es aritmética.

/// Rectángulo en puntos, origen arriba-izquierda.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_usable(&self) -> bool {
        self.w > 0.0 && self.h > 0.0 && self.w.is_finite() && self.h.is_finite()
    }
}

/// Escala de un eje.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Linear {
        min: f64,
        max: f64,
    },
    /// Base 10. Es la escala de la granulometría.
    Log10 {
        min: f64,
        max: f64,
    },
}

impl Scale {
    pub fn min(&self) -> f64 {
        match self {
            Self::Linear { min, .. } | Self::Log10 { min, .. } => *min,
        }
    }

    pub fn max(&self) -> f64 {
        match self {
            Self::Linear { max, .. } | Self::Log10 { max, .. } => *max,
        }
    }

    /// Posición normalizada en `0..=1`. Los valores fuera del dominio se recortan al
    /// borde en lugar de desaparecer: un punto que se sale no debe hacer desaparecer la
    /// serie entera.
    pub fn normalize(&self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        let (lo, hi) = (self.min(), self.max());
        if !(lo.is_finite() && hi.is_finite()) || hi <= lo {
            return None;
        }
        let t = match self {
            Self::Linear { .. } => (value - lo) / (hi - lo),
            Self::Log10 { .. } => {
                if lo <= 0.0 || value <= 0.0 {
                    return None;
                }
                (value.log10() - lo.log10()) / (hi.log10() - lo.log10())
            }
        };
        Some(t.clamp(0.0, 1.0))
    }

    /// Posición en píxeles sobre el eje X (crece hacia la derecha).
    pub fn x(&self, value: f64, plot: Rect) -> Option<f32> {
        self.normalize(value).map(|t| plot.x + (t as f32) * plot.w)
    }

    /// Posición en píxeles sobre el eje Y (crece hacia **abajo**: un valor mayor queda
    /// más arriba en pantalla).
    pub fn y(&self, value: f64, plot: Rect) -> Option<f32> {
        self.normalize(value)
            .map(|t| plot.y + (1.0 - t as f32) * plot.h)
    }
}

/// Una marca de eje ya posicionada.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Coordenada en píxeles (x para el eje horizontal, y para el vertical).
    pub at: f32,
    pub label: String,
}

/// Línea horizontal de referencia (límite de especificación, objetivo…).
#[derive(Clone, Debug, PartialEq)]
pub struct LimitLine {
    pub y: f32,
    pub label: String,
}

/// Gráfico de líneas ya calculado.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartGeometry {
    pub plot: Rect,
    pub line: Vec<[f32; 2]>,
    pub x_ticks: Vec<Tick>,
    pub y_ticks: Vec<Tick>,
    pub limits: Vec<LimitLine>,
}

impl ChartGeometry {
    pub fn is_empty(&self) -> bool {
        self.line.len() < 2
    }
}

/// Especificación de un gráfico de líneas.
#[derive(Clone, Debug)]
pub struct LineChartSpec<'a> {
    pub points: &'a [(f64, f64)],
    pub plot: Rect,
    pub x: Scale,
    pub y: Scale,
    pub x_ticks: Vec<f64>,
    pub y_ticks: Vec<f64>,
    pub limits: Vec<(f64, String)>,
}

/// Construye la geometría. Los puntos no representables se omiten, no se inventan.
pub fn line_chart(spec: &LineChartSpec<'_>) -> ChartGeometry {
    let line: Vec<[f32; 2]> = spec
        .points
        .iter()
        .filter_map(|(x, y)| Some([spec.x.x(*x, spec.plot)?, spec.y.y(*y, spec.plot)?]))
        .collect();

    let x_ticks = spec
        .x_ticks
        .iter()
        .filter_map(|v| {
            spec.x.x(*v, spec.plot).map(|at| Tick {
                at,
                label: format_tick(*v),
            })
        })
        .collect();

    let y_ticks = spec
        .y_ticks
        .iter()
        .filter_map(|v| {
            spec.y.y(*v, spec.plot).map(|at| Tick {
                at,
                label: format_tick(*v),
            })
        })
        .collect();

    let limits = spec
        .limits
        .iter()
        .filter_map(|(v, label)| {
            spec.y.y(*v, spec.plot).map(|y| LimitLine {
                y,
                label: label.clone(),
            })
        })
        .collect();

    ChartGeometry {
        plot: spec.plot,
        line,
        x_ticks,
        y_ticks,
        limits,
    }
}

/// Formatea una marca de eje con cifras razonables, sin ceros sobrantes.
///
/// Se conservan los decimales significativos: en una curva granulométrica el tamiz
/// 0,075 mm debe leerse «0.075», no «0.07» ni «0.08».
pub fn format_tick(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    let a = value.abs();
    if !(1e-4..1e7).contains(&a) {
        return format!("{value:.2e}");
    }
    let mut s = format!("{value:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// Marcas «bonitas» en escala lineal (paso 1, 2 o 5 por potencia de diez).
pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || target == 0 {
        return Vec::new();
    }
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let span = hi - lo;
    if span <= 0.0 {
        return vec![lo];
    }

    let raw_step = span / target as f64;
    let magnitude = 10f64.powf(raw_step.log10().floor());
    let normalized = raw_step / magnitude;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    } * magnitude;

    let start = (lo / step).floor() * step;
    let mut ticks = Vec::new();
    let mut i = 0i64;
    loop {
        let value = start + (i as f64) * step;
        if value > hi + step * 1e-9 {
            break;
        }
        if value >= lo - step * 1e-9 {
            // Re-snap para evitar 0.30000000000000004.
            ticks.push((value / step).round() * step);
        }
        i += 1;
        if i > 200 {
            break;
        }
    }
    ticks
}

/// Marcas por décadas dentro del rango (1, 10, 100, …).
pub fn log_ticks(min: f64, max: f64) -> Vec<f64> {
    if !(min.is_finite() && max.is_finite()) || min <= 0.0 || max <= min {
        return Vec::new();
    }
    let start = min.log10().ceil() as i32;
    let end = max.log10().floor() as i32;
    (start..=end).map(|e| 10f64.powi(e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 100.0)
    }

    #[test]
    fn escala_lineal_mapea_los_extremos() {
        let s = Scale::Linear {
            min: 0.0,
            max: 100.0,
        };
        assert_eq!(s.normalize(0.0), Some(0.0));
        assert_eq!(s.normalize(50.0), Some(0.5));
        assert_eq!(s.normalize(100.0), Some(1.0));
    }

    #[test]
    fn la_escala_logaritmica_mapea_decadas() {
        let s = Scale::Log10 {
            min: 1.0,
            max: 100.0,
        };
        assert_eq!(s.normalize(1.0), Some(0.0));
        let mid = s.normalize(10.0).unwrap();
        assert!((mid - 0.5).abs() < 1e-12, "10 cae justo en medio de 1..100");
        assert_eq!(s.normalize(100.0), Some(1.0));
    }

    #[test]
    fn la_escala_logaritmica_rechaza_cero_y_negativos() {
        let s = Scale::Log10 {
            min: 0.075,
            max: 100.0,
        };
        assert_eq!(s.normalize(0.0), None);
        assert_eq!(s.normalize(-1.0), None);
    }

    #[test]
    fn los_valores_fuera_de_rango_se_recortan_al_borde() {
        let s = Scale::Linear {
            min: 10.0,
            max: 20.0,
        };
        assert_eq!(s.normalize(5.0), Some(0.0));
        assert_eq!(s.normalize(25.0), Some(1.0));
    }

    #[test]
    fn un_dominio_degenerado_no_produce_posiciones() {
        let s = Scale::Linear { min: 5.0, max: 5.0 };
        assert_eq!(s.normalize(5.0), None);
    }

    #[test]
    fn el_eje_y_crece_hacia_abajo() {
        let s = Scale::Linear {
            min: 0.0,
            max: 10.0,
        };
        let y0 = s.y(0.0, plot()).unwrap();
        let y10 = s.y(10.0, plot()).unwrap();
        assert_eq!(y0, 100.0);
        assert_eq!(y10, 0.0);
        assert!(y10 < y0, "un valor mayor queda más arriba");
    }

    #[test]
    fn las_marcas_bonitas_van_de_cero_a_cien_de_veinte_en_veinte() {
        assert_eq!(
            nice_ticks(0.0, 100.0, 5),
            vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        );
    }

    #[test]
    fn las_marcas_evitan_la_coma_flotante() {
        for tick in nice_ticks(0.0, 1.0, 4) {
            let scaled = tick * 10.0;
            assert!(
                (scaled - scaled.round()).abs() < 1e-9,
                "marca con residuo flotante: {tick}"
            );
        }
    }

    #[test]
    fn las_marcas_logaritmicas_son_decadas() {
        assert_eq!(log_ticks(0.075, 75.0), vec![0.1, 1.0, 10.0]);
        assert!(log_ticks(0.0, 10.0).is_empty());
    }

    #[test]
    fn un_grafico_de_dos_puntos_cruza_las_esquinas() {
        let points = [(0.0, 0.0), (10.0, 100.0)];
        let spec = LineChartSpec {
            points: &points,
            plot: plot(),
            x: Scale::Linear {
                min: 0.0,
                max: 10.0,
            },
            y: Scale::Linear {
                min: 0.0,
                max: 100.0,
            },
            x_ticks: vec![0.0, 10.0],
            y_ticks: vec![0.0, 100.0],
            limits: vec![(50.0, "mínimo".into())],
        };
        let g = line_chart(&spec);
        assert_eq!(g.line, vec![[0.0, 100.0], [100.0, 0.0]]);
        assert_eq!(g.x_ticks.len(), 2);
        assert_eq!(g.y_ticks.len(), 2);
        assert_eq!(g.limits.len(), 1);
        assert_eq!(g.limits[0].y, 50.0);
    }

    #[test]
    fn una_serie_vacia_no_inventa_puntos() {
        let spec = LineChartSpec {
            points: &[],
            plot: plot(),
            x: Scale::Linear { min: 0.0, max: 1.0 },
            y: Scale::Linear { min: 0.0, max: 1.0 },
            x_ticks: vec![0.0, 1.0],
            y_ticks: vec![0.0, 1.0],
            limits: Vec::new(),
        };
        let g = line_chart(&spec);
        assert!(g.line.is_empty());
        assert!(g.is_empty());
        assert_eq!(g.x_ticks.len(), 2, "los ejes siguen existiendo");
    }

    #[test]
    fn un_punto_no_representable_se_omite_sin_romper_la_serie() {
        let points = [(1.0, 10.0), (0.0, 20.0), (10.0, 30.0)]; // el segundo es inválido en log
        let spec = LineChartSpec {
            points: &points,
            plot: plot(),
            x: Scale::Log10 {
                min: 0.1,
                max: 100.0,
            },
            y: Scale::Linear {
                min: 0.0,
                max: 40.0,
            },
            x_ticks: Vec::new(),
            y_ticks: Vec::new(),
            limits: Vec::new(),
        };
        let g = line_chart(&spec);
        assert_eq!(g.line.len(), 2);
    }

    #[test]
    fn las_etiquetas_de_las_marcas_son_legibles() {
        assert_eq!(format_tick(0.0), "0");
        assert_eq!(format_tick(20.0), "20");
        assert_eq!(format_tick(2.5), "2.5");
        // Tamiz 75 µm: debe conservar sus tres decimales, no redondearse a 0.07.
        assert_eq!(format_tick(0.075), "0.075");
        assert_eq!(format_tick(1234.5678), "1234.5678");
        assert_eq!(format_tick(0.000_05), "5.00e-5");
    }

    #[test]
    fn un_rectangulo_sin_area_no_es_utilizable() {
        assert!(!Rect::new(0.0, 0.0, 0.0, 100.0).is_usable());
        assert!(plot().is_usable());
    }
}
