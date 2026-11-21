-- Esquema v3: la base también rechaza unidades inventadas y valores no finitos.
--
-- Las mediciones y los resultados se validan en el dominio, pero una fila escrita por
-- otra vía (una versión anterior, una herramienta externa) no pasaría por esos
-- constructores. Y en `result` el daño sería permanente: la tabla no admite
-- modificaciones, así que un resultado con unidad «banana» o valor infinito quedaría ahí
-- para siempre y podría acabar en un informe.
--
-- Estos disparadores cierran esa puerta en el único punto que todas las escrituras
-- comparten: el INSERT.

CREATE TRIGGER measurement_unit_known
BEFORE INSERT ON measurement
WHEN NEW.unit NOT IN ('mm', 'cm', 'm', 'g', 'kg', 'N', 'kN', 'MPa', '%', '°C', 's')
BEGIN
    SELECT RAISE(ABORT, 'unidad no admitida en la medición');
END;

CREATE TRIGGER result_unit_known
BEFORE INSERT ON result
WHEN NEW.unit NOT IN ('mm', 'cm', 'm', 'g', 'kg', 'N', 'kN', 'MPa', '%', '°C', 's')
BEGIN
    SELECT RAISE(ABORT, 'unidad no admitida en el resultado');
END;

CREATE TRIGGER result_value_finite
BEFORE INSERT ON result
WHEN NEW.value IS NULL OR NEW.value > 1e308 OR NEW.value < -1e308
BEGIN
    SELECT RAISE(ABORT, 'un resultado debe ser un número finito');
END;
