# ADR-0010 · Generador de PDF propio en lugar de `printpdf`

- **Estado:** aceptado · **matiza ADR-0005**
- **Fecha:** 2026-11-21
- **Decisores:** arquitectura, informes

## Contexto

ADR-0005 eligió `printpdf` para los informes. Al llegar a la fase 2 se evaluó la versión
**0.12.11** y aparecieron tres problemas concretos:

1. **Documentación incompleta:** docs.rs marca el 56 % de la crate como documentado, y la
   API es una «segunda iteración» en movimiento (estructuras `PdfDocument`, `ops`,
   `graphics`, sin ejemplos claros de maquetación de texto).
2. **Árbol de dependencias desproporcionado:** arrastra `azul` (CSS, layout, fuentes),
   `lopdf`, `image`, `svg2pdf` y `rust-fontconfig` para generar una página de texto.
3. **Riesgo de bloqueo:** si la API cambia, el informe se rompe en un punto del producto
   que no admite excusas.

El informe que necesita el producto es de **maquetación fija**: A4, una columna,
Helvetica, líneas, reglas y saltos de página. No hay tablas que fluyan, ni imágenes, ni
saltos complejos.

## Decisión

Se implementa un **generador PDF mínimo propio** en `mamut-reports/src/pdf.rs`:

- PDF 1.4, fuentes base-14 (`Helvetica`, `Helvetica-Bold`), sin incrustar.
- Codificación **WinAnsi** para acentos y eñes, con escapes octales en el flujo y una
  tabla **`/ToUnicode`** para que el texto se pueda copiar y buscar.
- Texto, líneas y bandas de relleno. Nada más.
- Sin dependencias nuevas.
- Salida **determinista**: el mismo informe produce los mismos bytes.

La maquetación del informe vive en `layout.rs` y consume el mismo `ReportData` que el
renderizador de texto, de modo que previsualización y PDF no puedan divergir.

## Consecuencias

**Positivas**

- Cero dependencias: el árbol del producto no crece para escribir un PDF.
- Control total del resultado y de la paginación.
- El informe se puede inspeccionar con cualquier herramienta (flujo sin comprimir) y el
  texto se extrae correctamente, incluidos los acentos.
- Determinista: comparable byte a byte, útil para pruebas de regresión.

**Negativas**

- Es código propio que mantener: unas 400 líneas con 24 pruebas.
- **No admite imágenes, fuentes incrustadas ni tablas que fluyan.** Cuando el informe
  necesite el logo del laboratorio (fase 6), habrá que añadir imágenes XObject, o
  reconsiderar la decisión con el mismo criterio: medir antes de adoptar.
- La maquetación es manual: si el informe crece mucho, hay que cuidar la paginación.

## Verificación

- El archivo generado abre en visores y en `pymupdf`: 1 página, A4 exacta, texto extraído
  con acentos correctos (`Identificación`, `Recepción`, `Compresión`).
- 24 pruebas cubren: cabecera y tráiler, xref apuntando a objetos reales, escapes octales,
  WinAnsi, caracteres fuera de juego, paginación y contenido de cada sección.
- Ejemplo reproducible: `cargo run -p mamut-reports --example demo_pdf -- informe.pdf`.

## Qué se mantiene de ADR-0005

- El modelo `ReportData` + `ReportRenderer` sigue siendo la frontera.
- La regla de que el informe **imprime el veredicto que recibe** no cambia: si no se
  evaluó conformidad, el PDF lo dice.
- La evaluación de Typst como biblioteca queda descartada salvo que aparezca la necesidad
  de plantillas variables.
