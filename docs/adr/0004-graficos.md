# ADR-0004 · Gráficos técnicos: geometría propia sobre primitivas de Rune

- **Estado:** aceptado
- **Fecha:** 2026-11-18

## Contexto

El producto necesita gráficos: curvas de resistencia por edad, granulometría en
escala semilogarítmica, curvas de compactación (Proctor), CBR, límites de
Atterberg, y su inclusión en informes.

Rune **no incluye** un componente de gráficos. Sí incluye primitivas de dibujo
vectorial verificadas en `crates/rune/src/shape.rs`:

- `Shape::stroke(points, width, color)` — polilíneas.
- `Shape::fill(points, color)` — polígonos rellenos.
- `Shape::line(a, b, width, cap, color)` — segmentos con extremos.
- `Path` con `move_to`/`line_to`/`quad_to`/`cubic_to` — curvas.
- `Shape::view_box(w, h)` — sistema de coordenadas propio.
- `El::shape(shape)` y la función `canvas(shape)` para montarlo en la vista.

## Decisión

Se implementa un crate **`mamut-charts` sin dependencias de Rune**, que produce
geometría pura, y una capa fina en `mamut-app` que la convierte en `Shape`.

```rust
// mamut-charts: puro, testeable sin GPU ni ventana
pub struct Scale { /* Lineal | Log10 | Categorica */ }
pub struct Axis { pub title: String, pub unit: Unit, pub ticks: Vec<Tick> }
pub struct ChartGeometry {
    pub axes: [Axis; 2],
    pub polylines: Vec<Polyline>,   // Vec<[f32; 2]> en coordenadas del lienzo
    pub bands: Vec<Band>,           // p.ej. límite de especificación
    pub labels: Vec<Label>,
}
pub fn granulometry_curve(series: &[SieveResult], size: (f32, f32)) -> ChartGeometry;
```

En `mamut-app`, un adaptador de ~30 líneas traduce `ChartGeometry` a `Shape`.

## Motivos

- **Un gráfico de laboratorio no es un gráfico genérico.** Debe mostrar el límite de
  especificación como banda, la escala normativa (semilogarítmica en granulometría),
  las unidades en los ejes y las cifras significativas correctas. Las librerías
  genéricas obligan a pelearse con la API para conseguir eso.
- **Testeabilidad**: la geometría se prueba con `cargo test` sin abrir ventana
  (verificado en Fase 0: la función de serie devuelve puntos y se puede afirmar su
  monotonía y sus límites).
- **Independencia del framework**: si Rune se sustituye (ADR-0008), los gráficos se
  portan cambiando solo el adaptador.
- **Sin dependencias**: ninguna librería de gráficos en el árbol, ningún riesgo de
  licencia ni de API abandonada.

## Consecuencias

**Positivas**

- Control total de ejes, ticks, bandas, anotaciones y exportación.
- Gráficos idénticos en pantalla y en el PDF, porque ambos consumen la misma geometría.
- Sin dependencias nuevas.

**Negativas**

- Es trabajo propio: escalas, ticks «bonitos», colisiones de etiquetas, zoom y
  tooltips. Estimado: el componente base es acotado; las interacciones se añaden
  por necesidad, no de golpe.
- No habrá 3D, ni mapas, ni diagramas de dispersión animados salvo que se pidan.

## Alcance mínimo por fase

| Fase | Gráficos |
| --- | --- |
| 2 | Serie temporal simple (resistencia vs edad) con banda de especificación |
| 3 | Granulometría semilog, curva de compactación, CBR |
| 4 | Curvas de agregados, módulo de finura |
| 6 | Exportación de gráficos como imagen para insertar en informes |

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| `plotters` | Pensada para generar imágenes/backends, no para incrustar en un framework propio; obliga a rasterizar |
| `egui_plot` | Acoplada a `egui`: incompatible con Rune |
| Gráficos en el PDF y captura de pantalla en la app | Dos implementaciones distintas: divergen y una de las dos siempre está mal |
| WebView embebido para gráficos | Contradice la decisión de producto nativo |
