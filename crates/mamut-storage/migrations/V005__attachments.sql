-- Esquema v5: adjuntos direccionados por contenido.
--
-- El archivo se guarda en disco bajo su SHA-256; la base solo guarda la referencia y los
-- metadatos. Dos fotos idénticas ocupan una sola copia y el original nunca se modifica:
-- el nombre que ve el usuario es un dato de la fila, no del sistema de archivos.

CREATE TABLE attachment (
    id          TEXT PRIMARY KEY,
    sample_id   TEXT NOT NULL REFERENCES sample (id) ON DELETE RESTRICT,
    -- Nombre con el que el usuario conoce el archivo.
    file_name   TEXT NOT NULL,
    -- Tipo MIME declarado o detectado.
    mime        TEXT NOT NULL,
    -- SHA-256 del contenido: identifica el archivo en el almacén.
    sha256      TEXT NOT NULL,
    bytes       INTEGER NOT NULL CHECK (bytes > 0),
    -- Quién lo adjuntó y cuándo.
    added_by    TEXT NOT NULL,
    added_at    TEXT NOT NULL
);

CREATE INDEX idx_attachment_sample ON attachment (sample_id);
CREATE INDEX idx_attachment_sha ON attachment (sha256);

-- Un mismo archivo no se adjunta dos veces a la misma muestra (sí a muestras distintas).
CREATE UNIQUE INDEX idx_attachment_unique ON attachment (sample_id, sha256);
