-- Esquema v1 de MAMUT Lab.
--
-- Dos garantías se imponen aquí y no en el código de aplicación:
--   1. El registro de auditoría es append-only (disparadores).
--   2. Un resultado calculado no se modifica ni se borra (disparadores).
--
-- La tabla `schema_version` la crea el migrador antes de aplicar nada.

CREATE TABLE sample (
    id                TEXT PRIMARY KEY,
    code              TEXT NOT NULL UNIQUE,
    material          TEXT NOT NULL,
    origin            TEXT NOT NULL,
    client            TEXT NOT NULL,
    project           TEXT NOT NULL,
    received_on       TEXT NOT NULL,
    responsible_id    TEXT NOT NULL,
    responsible_name  TEXT NOT NULL,
    state             TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    revision          INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_sample_received ON sample (received_on);
CREATE INDEX idx_sample_state ON sample (state);

CREATE TABLE specimen (
    id           TEXT PRIMARY KEY,
    sample_id    TEXT NOT NULL REFERENCES sample (id) ON DELETE RESTRICT,
    designation  TEXT NOT NULL,
    diameter_mm  REAL NOT NULL CHECK (diameter_mm > 0),
    height_mm    REAL NOT NULL CHECK (height_mm > 0),
    mass_g       REAL NOT NULL CHECK (mass_g > 0),
    age_days     INTEGER NOT NULL CHECK (age_days >= 0),
    created_at   TEXT NOT NULL,
    UNIQUE (sample_id, designation)
);

CREATE INDEX idx_specimen_sample ON specimen (sample_id);

CREATE TABLE test_record (
    id               TEXT PRIMARY KEY,
    specimen_id      TEXT NOT NULL REFERENCES specimen (id) ON DELETE RESTRICT,
    test_code        TEXT NOT NULL,
    -- El descriptor completo (norma, método, estado de revisión) como JSON:
    -- es un objeto de valor y no se consulta por partes en esta fase.
    descriptor_json  TEXT NOT NULL,
    state            TEXT NOT NULL,
    performed_on     TEXT NOT NULL,
    equipment        TEXT,
    operator_id      TEXT NOT NULL,
    operator_name    TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    revision         INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_test_specimen ON test_record (specimen_id);
CREATE INDEX idx_test_code ON test_record (test_code);

CREATE TABLE measurement (
    id                TEXT PRIMARY KEY,
    test_id           TEXT NOT NULL REFERENCES test_record (id) ON DELETE RESTRICT,
    variable          TEXT NOT NULL,
    value_text        TEXT NOT NULL,
    unit              TEXT NOT NULL,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    supersedes        INTEGER,
    correction_reason TEXT,
    recorded_at       TEXT NOT NULL,
    UNIQUE (test_id, variable, revision),
    -- Una corrección debe explicar por qué existe.
    CHECK (supersedes IS NULL OR correction_reason IS NOT NULL)
);

CREATE INDEX idx_measurement_test ON measurement (test_id);

CREATE TABLE result (
    id                 TEXT PRIMARY KEY,
    test_id            TEXT NOT NULL REFERENCES test_record (id) ON DELETE RESTRICT,
    value              REAL NOT NULL,
    unit               TEXT NOT NULL,
    engine_version     TEXT NOT NULL,
    inputs_fingerprint TEXT NOT NULL,
    calculated_at      TEXT NOT NULL
);

CREATE INDEX idx_result_test ON result (test_id);

CREATE TABLE equipment (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,
    brand               TEXT,
    model               TEXT,
    serial              TEXT,
    acquired_on         TEXT,
    calibrated_on       TEXT,
    next_calibration_on TEXT,
    responsible         TEXT,
    state               TEXT NOT NULL DEFAULT 'active'
);

CREATE INDEX idx_equipment_next_calibration ON equipment (next_calibration_on);

CREATE TABLE audit (
    id          TEXT PRIMARY KEY,
    at          TEXT NOT NULL,
    actor_id    TEXT NOT NULL,
    actor_name  TEXT NOT NULL,
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    action      TEXT NOT NULL,
    detail      TEXT
);

CREATE INDEX idx_audit_entity ON audit (entity, entity_id);
CREATE INDEX idx_audit_at ON audit (at);

-- ---------------------------------------------------------------------------
-- Inmutabilidad impuesta por la base de datos.
-- ---------------------------------------------------------------------------

CREATE TRIGGER audit_no_update
BEFORE UPDATE ON audit
BEGIN
    SELECT RAISE(ABORT, 'el registro de auditoría es append-only: no se modifica');
END;

CREATE TRIGGER audit_no_delete
BEFORE DELETE ON audit
BEGIN
    SELECT RAISE(ABORT, 'el registro de auditoría es append-only: no se borra');
END;

CREATE TRIGGER result_no_update
BEFORE UPDATE ON result
BEGIN
    SELECT RAISE(ABORT, 'un resultado calculado no se modifica: emita uno nuevo');
END;

CREATE TRIGGER result_no_delete
BEFORE DELETE ON result
BEGIN
    SELECT RAISE(ABORT, 'un resultado calculado no se borra: use una anulación auditada');
END;
