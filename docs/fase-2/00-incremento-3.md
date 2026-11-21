# Fase 2 · Incremento 3 — Cierre de la fase

**Fecha:** 2026-11-21 · **Estado:** fase 2 cerrada

## Qué faltaba y ya está

| Pendiente | Cómo quedó |
| --- | --- |
| **Adjuntos** | `Store::attach_file` copia el contenido a `<datos>/attachments/<sha256>/`, registra metadatos (nombre, MIME, hash, tamaño) y deja una entrada de auditoría. Si el mismo archivo se adjunta dos veces a la misma muestra, avisa en vez de duplicarlo |
| **Gráfico resistencia–edad** | `Store::series_for_sample` obtiene edad y resultado de cada ensayo; `mamut-charts` calcula la geometría y Rune la dibuja. Con menos de dos edades distintas no se dibuja nada: un gráfico de un punto no es un gráfico |
| **Exportación** | Botones CSV y XLSX sobre el listado de muestras; los archivos van a `<datos>/exports/` |
| **Anulación** | Botón «Anular» con motivo obligatorio: la muestra pasa a `Anulada`, se conserva y la auditoría guarda el motivo |

Con esto, los cuatro entregables que faltaban de la fase 2 están implementados y probados.

## Nota sobre «editar»

La anulación cubre la trazabilidad (no se borra: se marca). **Editar** los datos de una
muestra existente no se implementó porque el roadmap no lo pedía y porque, según la regla
del producto, una corrección debe crear una versión con motivo, no sobrescribir. Queda
como decisión de producto para la fase 5 (trazabilidad y auditoría), no como olvido.

## Evidencia

```
cargo fmt --check        → 0
cargo clippy -D warnings → 0
cargo test --workspace   → 306 pruebas, 0 fallos
```

Pruebas nuevas: adjuntar copia el archivo y detecta duplicados, anular cambia el estado y
registra auditoría, exportar escribe CSV y XLSX reales, y dos ensayos de la misma muestra
dibujan el gráfico con clics simulados en la interfaz.

## Estado de la fase 2

**Cerrada.** El ciclo completo —alta de muestra, ensayo de compresión, resultado,
historial e informe PDF— funciona y está probado, con los cuatro entregables restantes ya
implementados. Lo único que sigue abierto es la identidad del laboratorio en el informe
(DT-23) y las deudas transversales (DT-12, DT-13, DT-19…), que no pertenecen a esta fase.
