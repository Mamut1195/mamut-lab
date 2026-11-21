//! Generador de PDF mínimo, determinista y sin dependencias.
//!
//! **Por qué no `printpdf`.** La versión 0.12 está documentada al 56 %, su API es una
//! «segunda iteración» en movimiento y arrastra `azul` y `lopdf` (árbol de dependencias
//! grande para lo que necesitamos). Un informe técnico es una maquetación **fija**: A4,
//! Helvetica, líneas, reglas y saltos de página. Escribir eso directamente produce un
//! archivo válido, idéntico en cada ejecución y sin dependencias (ADR-0010).
//!
//! **Qué genera.** PDF 1.4 con fuentes base-14 (Helvetica y Helvetica-Bold, sin incrustar),
//! codificación WinAnsi para acentos y eñes, texto, líneas y rellenos. Nada más: si algún
//! día hace falta una imagen o una fuente propia, se añade aquí con su prueba.
//!
//! **Coordenadas.** La API trabaja en milímetros con el origen **arriba a la izquierda**
//! (como una pantalla); la conversión a la coordenada PDF (origen abajo) se hace dentro.

use std::fmt::Write as _;

/// Ancho de A4 en milímetros.
pub const A4_WIDTH_MM: f32 = 210.0;
/// Alto de A4 en milímetros.
pub const A4_HEIGHT_MM: f32 = 297.0;

/// Milímetros a puntos PostScript (1 in = 25.4 mm = 72 pt).
fn mm_to_pt(mm: f32) -> f32 {
    mm * 72.0 / 25.4
}

/// Convierte una `y` medida desde arriba a la coordenada PDF (desde abajo).
fn flip_y(y_mm: f32) -> f32 {
    mm_to_pt(A4_HEIGHT_MM - y_mm)
}

/// Escapa una cadena y la codifica en WinAnsi (cp1252).
///
/// Los caracteres fuera de WinAnsi se sustituyen por `?`: es preferible un carácter
/// perdido a un PDF que no abre.
fn winansi(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let byte = match ch {
            '\u{2013}' => 0x96, // –
            '\u{2014}' => 0x97, // —
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93,
            '\u{201D}' => 0x94,
            '\u{2022}' => 0x95, // •
            '\u{20AC}' => 0x80, // €
            c if (c as u32) <= 0xFF => c as u32 as u8,
            _ => b'?',
        };
        out.push(byte);
    }
    out
}

/// Convierte el texto en un literal PDF seguro.
///
/// **Por qué no se escribe el texto tal cual.** Una cadena Rust es UTF-8; un PDF con
/// `WinAnsiEncoding` espera bytes cp1252. Pasar por `String` intermedio destruía los
/// acentos (el byte 0xF3 no es UTF-8 válido y se convertía en «�»). Aquí cada byte se
/// emite como carácter ASCII imprimible o como escape octal `\363`, que es ASCII puro y
/// el visor interpreta como el byte correcto.
fn pdf_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for byte in winansi(text) {
        match byte {
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7E => out.push(byte as char),
            other => out.push_str(&format!("\\{other:03o}")),
        }
    }
    out
}

/// Construye la tabla `/ToUnicode` (CMap) que traduce cada byte WinAnsi a su carácter.
///
/// Sin esto el PDF se ve bien pero **el texto no se puede copiar**: los visores no saben
/// qué carácter hay detrás de cada byte y devuelven «�». Para un informe técnico eso es
/// inaceptable: el cliente copia la norma o el resultado a otro documento.
fn to_unicode_cmap() -> String {
    let mut map = String::from(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<20> <FF>\nendcodespacerange\n",
    );

    let mut entries: Vec<String> = Vec::new();
    for code in 0x20u8..=0xFF {
        // cp1252 deja cinco posiciones sin definir.
        if matches!(code, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D) {
            continue;
        }
        let unicode: u16 = match code {
            0x80 => 0x20AC,
            0x82 => 0x201A,
            0x83 => 0x0192,
            0x84 => 0x201E,
            0x85 => 0x2026,
            0x86 => 0x2020,
            0x87 => 0x2021,
            0x88 => 0x02C6,
            0x89 => 0x2030,
            0x8A => 0x0160,
            0x8B => 0x2039,
            0x8C => 0x0152,
            0x8E => 0x017D,
            0x91 => 0x2018,
            0x92 => 0x2019,
            0x93 => 0x201C,
            0x94 => 0x201D,
            0x95 => 0x2022,
            0x96 => 0x2013,
            0x97 => 0x2014,
            0x98 => 0x02DC,
            0x99 => 0x2122,
            0x9A => 0x0161,
            0x9B => 0x203A,
            0x9C => 0x0153,
            0x9E => 0x017E,
            0x9F => 0x0178,
            other => u16::from(other),
        };
        entries.push(format!("<{code:02X}> <{unicode:04X}>"));
    }

    // La especificación limita cada bloque a 100 entradas.
    for chunk in entries.chunks(100) {
        map.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for entry in chunk {
            map.push_str(entry);
            map.push('\n');
        }
        map.push_str("endbfchar\n");
    }

    map.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    map
}

/// Una página en construcción: su flujo de contenido ya en sintaxis PDF.
struct Page {
    content: String,
}

/// Documento en construcción.
pub struct PdfDocument {
    title: String,
    pages: Vec<Page>,
}

impl PdfDocument {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            pages: vec![Page {
                content: String::new(),
            }],
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Añade una página y pasa a escribir en ella.
    pub fn add_page(&mut self) {
        self.pages.push(Page {
            content: String::new(),
        });
    }

    fn current(&mut self) -> &mut String {
        // Un documento siempre tiene al menos una página; aun así no se usa `expect`:
        // si el vector estuviera vacío por un cambio futuro, se crea la página.
        if self.pages.is_empty() {
            self.pages.push(Page {
                content: String::new(),
            });
        }
        let index = self.pages.len() - 1;
        &mut self.pages[index].content
    }

    /// Escribe texto. `y_mm` es la línea base, medida desde arriba.
    pub fn text(&mut self, x_mm: f32, y_mm: f32, size_pt: f32, bold: bool, text: &str) {
        let font = if bold { "/F2" } else { "/F1" };
        let literal = pdf_literal(text);
        let op = format!(
            "BT {font} {size_pt} Tf 1 0 0 1 {:.2} {:.2} Tm ({literal}) Tj ET\n",
            mm_to_pt(x_mm),
            flip_y(y_mm)
        );
        self.current().push_str(&op);
    }

    /// Línea horizontal o vertical fina, en gris.
    pub fn line(&mut self, x1_mm: f32, y1_mm: f32, x2_mm: f32, y2_mm: f32) {
        let op = format!(
            "q 0.5 w 0.55 G {:.2} {:.2} m {:.2} {:.2} l S Q\n",
            mm_to_pt(x1_mm),
            flip_y(y1_mm),
            mm_to_pt(x2_mm),
            flip_y(y2_mm)
        );
        self.current().push_str(&op);
    }

    /// Rectángulo relleno en gris claro, para bandas de tabla.
    pub fn band(&mut self, x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32) {
        let op = format!(
            "q 0.93 g {:.2} {:.2} {:.2} {:.2} re f Q\n",
            mm_to_pt(x_mm),
            flip_y(y_mm + h_mm),
            mm_to_pt(w_mm),
            mm_to_pt(h_mm)
        );
        self.current().push_str(&op);
    }

    /// Serializa el documento a bytes PDF 1.4.
    pub fn save(self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        // Cabecera con marca binaria (recomendada por la especificación).
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

        // Objetos 1..4 fijos: catálogo, árbol de páginas, Helvetica, Helvetica-Bold.
        let mut offsets: Vec<usize> = vec![0]; // índice 0 sin usar (los objetos empiezan en 1)

        let push_object = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
            offsets.push(out.len());
            out.extend_from_slice(body);
        };

        let page_count = self.pages.len();
        // Cada página ocupa dos objetos: Page y Contents. Los objetos 1–5 son fijos
        // (catálogo, árbol, dos fuentes y la tabla ToUnicode).
        let kids: Vec<String> = (0..page_count)
            .map(|index| format!("{} 0 R", 6 + index * 2))
            .collect();

        push_object(
            &mut out,
            &mut offsets,
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        );
        push_object(
            &mut out,
            &mut offsets,
            format!(
                "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {page_count} >>\nendobj\n",
                kids.join(" ")
            )
            .as_bytes(),
        );
        push_object(
            &mut out,
            &mut offsets,
            b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /ToUnicode 5 0 R >>\nendobj\n",
        );
        push_object(
            &mut out,
            &mut offsets,
            b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding /ToUnicode 5 0 R >>\nendobj\n",
        );

        // Objeto 5: tabla ToUnicode, compartida por las dos fuentes.
        let cmap = to_unicode_cmap();
        let mut cmap_object = Vec::with_capacity(cmap.len() + 64);
        cmap_object.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", cmap.len()).as_bytes(),
        );
        cmap_object.extend_from_slice(cmap.as_bytes());
        cmap_object.extend_from_slice(b"endstream\nendobj\n");
        push_object(&mut out, &mut offsets, &cmap_object);

        let title = String::from_utf8_lossy(&winansi(&self.title)).into_owned();
        for (index, page) in self.pages.iter().enumerate() {
            let page_id = 6 + index * 2;
            let content_id = page_id + 1;

            let page_body = format!(
                "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R \
                 /MediaBox [0 0 {:.2} {:.2}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> \
                 /Contents {content_id} 0 R >>\nendobj\n",
                mm_to_pt(A4_WIDTH_MM),
                mm_to_pt(A4_HEIGHT_MM)
            );
            push_object(&mut out, &mut offsets, page_body.as_bytes());

            // El flujo se escribe sin comprimir: el informe es pequeño y así se puede
            // inspeccionar con cualquier herramienta.
            let content = &page.content;
            let mut stream = Vec::with_capacity(content.len() + 64);
            let header = format!(
                "{content_id} 0 obj\n<< /Length {} >>\nstream\n",
                content.len()
            );
            stream.extend_from_slice(header.as_bytes());
            stream.extend_from_slice(content.as_bytes());
            stream.extend_from_slice(b"endstream\nendobj\n");
            push_object(&mut out, &mut offsets, &stream);
        }

        // Información del documento (opcional pero útil).
        let info_id = 6 + page_count * 2;
        let info =
            format!("{info_id} 0 obj\n<< /Title ({title}) /Producer (MAMUT Lab) >>\nendobj\n");
        push_object(&mut out, &mut offsets, info.as_bytes());

        // Tabla xref y tráiler.
        let xref_offset = out.len();
        let total = offsets.len(); // incluye el índice 0
        let mut xref = format!("xref\n0 {total}\n0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            let _ = writeln!(xref, "{offset:010} 00000 n ");
        }
        out.extend_from_slice(xref.as_bytes());
        let trailer = format!(
            "trailer\n<< /Size {total} /Root 1 0 R /Info {info_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
        );
        out.extend_from_slice(trailer.as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn un_documento_vacio_es_un_pdf_valido() {
        let doc = PdfDocument::new("Prueba");
        let bytes = doc.save();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(text_of(&bytes).trim_end().ends_with("%%EOF"));
        assert!(text_of(&bytes).contains("/Type /Catalog"));
        assert!(text_of(&bytes).contains("startxref"));
    }

    #[test]
    fn el_texto_aparece_en_el_flujo() {
        let mut doc = PdfDocument::new("Prueba");
        doc.text(20.0, 30.0, 12.0, false, "Hormigón H-2026-0001");
        let text = text_of(&doc.save());
        assert!(text.contains("H-2026-0001"));
    }

    #[test]
    fn los_acentos_se_codifican_en_winansi() {
        // «ó» debe salir como un solo byte 0xF3, no como dos bytes UTF-8.
        let bytes = winansi("Hormigón");
        assert!(bytes.contains(&0xF3), "se esperaba la ó en cp1252");
        assert_eq!(bytes.len(), 8, "8 caracteres, 8 bytes");
    }

    #[test]
    fn el_literal_pdf_mantiene_el_texto_ascii_legible() {
        // Lo que puede quedarse en ASCII se queda: así el flujo se puede leer e inspeccionar.
        assert_eq!(pdf_literal("H-2026-0001"), "H-2026-0001");
    }

    #[test]
    fn el_literal_pdf_escapa_los_acentos_en_octal() {
        // 0xF3 = 363 en octal. Es ASCII puro, así que no rompe el archivo.
        assert_eq!(pdf_literal("ó"), "\\363");
        assert!(pdf_literal("Hormigón").ends_with("\\363n"));
    }

    #[test]
    fn los_parentesis_y_barras_se_escapan() {
        let text = pdf_literal("f'c (diseño) \\ 21");
        assert!(text.contains("\\("), "el paréntesis debe escaparse");
        assert!(text.contains("\\)"));
        assert!(text.contains("\\\\"), "la barra debe escaparse");
    }

    #[test]
    fn un_caracter_fuera_de_winansi_no_rompe_el_pdf() {
        let text = pdf_literal("temperatura 25 °C 😀");
        assert!(
            text.contains('?'),
            "el emoji se sustituye, no se pierde el PDF"
        );
    }

    #[test]
    fn varias_paginas_generan_varios_objetos_de_pagina() {
        let mut doc = PdfDocument::new("Prueba");
        doc.text(20.0, 30.0, 12.0, false, "uno");
        doc.add_page();
        doc.text(20.0, 30.0, 12.0, false, "dos");
        assert_eq!(doc.page_count(), 2);
        let text = text_of(&doc.save());
        assert!(text.contains("/Count 2"));
        assert!(text.contains("(uno)"));
        assert!(text.contains("(dos)"));
    }

    #[test]
    fn la_tabla_xref_apunta_a_objetos_reales() {
        let mut doc = PdfDocument::new("Prueba");
        doc.text(20.0, 30.0, 12.0, false, "uno");
        let bytes = doc.save();
        let text = text_of(&bytes);

        // La tabla se localiza sobre el texto, pero los desplazamientos se comprueban
        // sobre los **bytes**: el flujo contiene WinAnsi y no es UTF-8 válido.
        let xref_start = text.find("xref\n0 ").expect("tabla xref");
        let table = &text[xref_start..];
        let mut offsets = Vec::new();
        for line in table.lines().skip(2) {
            if line.len() >= 18 && line.ends_with(" n ") {
                if let Ok(value) = line[..10].trim().parse::<usize>() {
                    offsets.push(value);
                }
            } else if line.starts_with("trailer") {
                break;
            }
        }
        assert!(
            offsets.len() >= 5,
            "se esperaban varios objetos: {offsets:?}"
        );
        for offset in offsets {
            assert!(
                bytes[offset].is_ascii_digit(),
                "la xref apunta a un byte que no inicia objeto: {}",
                bytes[offset] as char
            );
        }
    }
}
