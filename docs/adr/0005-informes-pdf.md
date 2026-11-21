# ADR-0005 · Informes técnicos en PDF

- **Estado:** aceptado (crate pendiente de prueba de concepto en fase 2)
- **Fecha:** 2026-11-18

## Contexto

Los informes son el entregable visible del laboratorio: deben incluir identificación
del laboratorio, de la muestra, datos del ensayo, método, norma y versión, datos
originales, cálculos, resultados, gráficos, observaciones, responsable y fecha.
Deben ser profesionales y permitir personalizar la identidad visual.

Rune no genera PDF. Se necesita una crate de Rust.

## Decisión

Se usa **`printpdf` 0.12** para componer el PDF de forma programática, con una capa
de plantillas propia en `mamut-reports`.

- `mamut-reports` **no** depende de Rune: recibe un `ReportData` (dominio +
  geometría de gráficos ya calculada) y produce bytes.
- Los gráficos se insertan como **vectores** generados desde `mamut-charts`, no como
  capturas de pantalla, para que el PDF sea nítido a cualquier zoom e imprimible.
- La identidad visual (logo, datos del laboratorio, firma, colores) es un recurso
  editable en la configuración, no código.
- El PDF generado se **versiona** junto al `CalculationRecord`: un informe antiguo se
  puede reproducir.

Verificado en Fase 0: `printpdf 0.12.11` **resuelve y compila** en este toolchain y
plataforma (ver `02-validacion-stack.md`). Su API concreta se validará con una prueba
de concepto al inicio de la fase 2, antes de comprometerse con la plantilla.

## Motivos

- **Rust puro, sin dependencias de sistema**: coherente con el requisito offline y con
  la distribución en Windows sin instalar nada.
- **Control de bajo nivel** sobre posiciones, fuentes y vectores: necesario para un
  informe técnico con tablas densas y gráficos exactos.
- **Sin proceso externo**: nada de invocar un navegador headless ni un motor LaTeX.

## Riesgos y plan de contingencia

| Riesgo | Probabilidad | Mitigación |
| --- | --- | --- |
| La API de `printpdf` 0.12 es incómoda para maquetar tablas | Media | Se aísla tras un trait `PdfRenderer`; si estorba, se sustituye sin tocar los informes |
| Se necesita maquetación declarativa avanzada (saltos de página automáticos, tablas que fluyen) | Media | Evaluar **Typst como biblioteca** (`typst` + `typst-pdf`) para plantillas en su lenguaje; decisión al final de la fase 2 |
| Fuentes: el informe debe incrustar una fuente con tildes y ñ | Baja | Incrustar una fuente OFL (p. ej. Inter, ya usada por Rune, o una serif para informes) |
| Requisito de firma digital del informe | Baja (fase 6) | Preparar `ReportData` con un hash canónico para poder firmar después |

## Consecuencias

**Positivas**

- Informes reproducibles y diffeables en control de versiones (texto + vectores).
- Sin dependencias externas ni licencias adicionales.

**Negativas**

- Maquetar tablas en `printpdf` es trabajo manual: hay que construir utilidades de
  tabla, saltos de página y cabeceras repetidas.
- La calidad tipográfica final depende de nuestro código, no de un motor de
  maquetación.

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| `genpdf` | Abstracción de alto nivel sobre `printpdf`; menos control y mantenimiento incierto |
| `wkhtmltopdf` / Chrome headless | Proceso externo, dependencia de sistema, no offline puro |
| LaTeX embebido | Enorme, licencias y distribución complejas |
| HTML → PDF en Rust | Ecosistema inmaduro para maquetación técnica fiable |
| Exportar solo XLSX/CSV | No cumple el requisito de informe técnico |
