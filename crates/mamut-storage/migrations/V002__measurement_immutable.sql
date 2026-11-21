-- Esquema v2: las mediciones también son inmutables.
--
-- La v1 solo obligaba a que una corrección declarara su motivo (CHECK). Eso no impedía
-- lo importante: un `UPDATE measurement SET value_text = ...` borraba el dato original
-- sin dejar rastro, que es exactamente lo que la regla 4 del encargo prohíbe.
--
-- A partir de la v2, corregir exige INSERTAR una versión nueva (revision + 1,
-- supersedes = revisión anterior, correction_reason obligatorio).

CREATE TRIGGER measurement_no_update
BEFORE UPDATE ON measurement
BEGIN
    SELECT RAISE(ABORT, 'una medición no se modifica: registre una versión corregida');
END;

CREATE TRIGGER measurement_no_delete
BEFORE DELETE ON measurement
BEGIN
    SELECT RAISE(ABORT, 'una medición no se borra: registre una versión corregida');
END;
