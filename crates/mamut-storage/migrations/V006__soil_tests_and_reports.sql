-- Esquema v6: ensayos de suelos y registro de informes.
--
-- Hasta aquí un ensayo de suelos vivía en el PDF y en ningún otro sitio: si el archivo se
-- perdía o se alteraba no quedaba evidencia. Estas dos tablas cierran ese hueco.
--
-- `soil_test.inputs` guarda los datos originales en JSON, tal como se capturaron. No se
-- normaliza a columnas porque cada ensayo tiene su propia forma (tamices, golpes, curva de
-- compactación, curva de penetración) y forzarlos a un esquema común perdería datos.

CREATE TABLE soil_test (
    id              TEXT PRIMARY KEY,
    sample_id       TEXT NOT NULL REFERENCES sample (id) ON DELETE RESTRICT,
    -- 'granulometria' | 'atterberg' | 'compactacion' | 'cbr' | 'densidad'
    kind            TEXT NOT NULL,
    descriptor_code TEXT NOT NULL,
    -- Datos originales exactos del ensayo.
    inputs          TEXT NOT NULL,
    result_value    REAL NOT NULL,
    result_unit     TEXT NOT NULL,
    engine_version  TEXT NOT NULL,
    -- Huella de los datos de entrada y del motor.
    fingerprint     TEXT NOT NULL,
    performed_on    TEXT NOT NULL,
    created_by      TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE INDEX idx_soil_test_sample ON soil_test (sample_id);
CREATE INDEX idx_soil_test_kind ON soil_test (kind);

-- Los ensayos de suelos son inmutables una vez guardados: se corrige con uno nuevo.
CREATE TRIGGER soil_test_no_update
BEFORE UPDATE ON soil_test
BEGIN
    SELECT RAISE(ABORT, 'un ensayo de suelos guardado no se modifica: se crea otro');
END;

CREATE TRIGGER soil_test_no_delete
BEFORE DELETE ON soil_test
BEGIN
    SELECT RAISE(ABORT, 'un ensayo de suelos guardado no se borra');
END;

-- Registro de informes emitidos: qué se emitió, sobre qué, cuándo y con qué contenido.
CREATE TABLE report (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    -- Identificador del ensayo (test_record o soil_test) o de la muestra.
    subject    TEXT NOT NULL,
    sample_id  TEXT NOT NULL REFERENCES sample (id) ON DELETE RESTRICT,
    path       TEXT NOT NULL,
    -- SHA-256 del PDF emitido, para detectar sustituciones.
    sha256     TEXT NOT NULL,
    issued_by  TEXT NOT NULL,
    issued_at  TEXT NOT NULL
);

CREATE INDEX idx_report_subject ON report (subject);
CREATE INDEX idx_report_sample ON report (sample_id);

CREATE TRIGGER report_no_update
BEFORE UPDATE ON report
BEGIN
    SELECT RAISE(ABORT, 'un informe emitido no se modifica');
END;

CREATE TRIGGER report_no_delete
BEFORE DELETE ON report
BEGIN
    SELECT RAISE(ABORT, 'un informe emitido no se borra');
END;
