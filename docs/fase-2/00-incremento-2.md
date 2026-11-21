# Fase 2 · Incremento 2 — Ensayo de compresión e informe

**Fecha:** 2026-11-21 · **Estado:** completado y probado

## Objetivo

Cerrar el ciclo que define la fase: **muestra → ensayo → informe**.

## Qué se ha implementado

| Pieza | Dónde | Qué hace |
| --- | --- | --- |
| Espacio de ensayo | `mamut-app/src/view.rs`, `state.rs` | Formulario de compresión: muestra, designación, edad, dimensiones, masa y carga |
| Cálculo | `AppState::calculate_test` | Valida con `mamut-validation`, calcula con `mamut-calculations`, muestra resultado, esbeltez y veredicto |
| Persistencia del ciclo | `Store::save_test_bundle` | Espécimen, ensayo, **mediciones originales** y resultado, en el orden que exigen las claves foráneas y el dominio; con tres entradas de auditoría |
| Historial | `Store::tests_for_sample` + `TestRow` | Ensayos de la muestra con fecha, código, resultado y estado |
| Informe PDF | `mamut-reports` (`pdf.rs` + `layout.rs`) | Generador propio, A4, Helvetica, WinAnsi con `/ToUnicode`; 24 pruebas |
| Botón «Informe PDF» | `AppState::generate_report` | Compone, genera y guarda en `<datos>/reports/informe-<muestra>.pdf` |

## Decisiones que importan

- **Calcular no exige que la muestra exista; guardar sí.** La aritmética es válida con
  cualquier número; un ensayo, en cambio, pertenece a una muestra registrada. Hay una
  prueba para cada mitad.
- **La coma decimal se acepta.** `150,5` es como se teclea en español; convertirlo a
  `150.5` es trabajo del producto, no del usuario.
- **Los datos originales se guardan tal como se escribieron**, separados del resultado
  calculado. Si mañana se corrige un dígito, se crea una versión, no se sobrescribe.
- **El veredicto se imprime tal cual llega.** Como la norma del pack sigue en
  `PendingReview`, el informe dice «SIN JUICIO DE CONFORMIDAD» y no hay ninguna rama que
  lo convierta en «conforme».
- **No se usa `printpdf`**: ver ADR-0010. El generador propio no añade dependencias y el
  PDF resultante se abre, se imprime y se copia correctamente.

## Evidencia

```
cargo fmt --check        → 0
cargo clippy -D warnings → 0
cargo test --workspace   → 303 pruebas, 0 fallos
```

- Pruebas del armazón (19): el ciclo completo se ejecuta con clics simulados —calcular,
  guardar, ver historial y **generar un PDF que existe en disco y empieza por `%PDF`**—,
  además de los rechazos: guardar sin calcular, muestra inexistente, carga cero.
- Pruebas del PDF (24): estructura, xref, escapes, paginación y contenido de secciones.
- Captura: [`fase-2-ensayo.png`](fase-2-ensayo.png), con un ensayo real calculado
  (500 kN sobre 150 mm = 28,29 MPa, esbeltez 2,00).

## Lo que este incremento **no** hace todavía

| Pendiente | Estado |
| --- | --- |
| Adjuntos (fotos y documentos) | Tabla creada en el esquema v5; falta el almacén de archivos y la interfaz (DT-22) |
| Gráfico resistencia–edad | La geometría existe en `mamut-charts`; falta montarlo (necesita dos ensayos de la misma muestra) |
| Botón de exportación CSV/XLSX | Los exportadores existen y están probados; falta la acción en pantalla |
| Editar o anular muestras | Solo alta |
| Identidad del laboratorio en el informe | Hoy dice «MAMUT Lab»; debe configurarse por instalación (DT-23) |

Ninguno de estos bloquea el ciclo, que es el criterio de salida de la fase.

## Siguiente

Cerrar los pendientes de arriba en un incremento 3, y empezar la fase 3 (suelos) con las
ecuaciones ya documentadas en `docs/normas/referencias-publicas/`.
