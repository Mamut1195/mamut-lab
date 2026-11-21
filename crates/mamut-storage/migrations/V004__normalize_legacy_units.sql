-- Esquema v4: normaliza los símbolos de unidad heredados.
--
-- Una base creada con la v1 pudo guardar unidades escritas a mano («milímetros»,
-- «KGS», «Mpa»). A partir de la v3 la lectura las rechaza —correctamente, porque no son
-- símbolos canónicos—, pero como las mediciones y los resultados son inmutables no había
-- forma de repararlas: quedaban conservadas pero ilegibles.
--
-- Esta migración las normaliza al símbolo canónico. Es la única operación que toca filas
-- existentes, y solo cambia la **forma de escribir la unidad**, nunca el valor medido:
-- «milímetros» → «mm», «KGS» → «kg», «mpa» → «MPa».
--
-- Los disparadores de inmutabilidad se retiran y se vuelven a crear dentro de la misma
-- transacción: la migración es una operación de esquema, no una edición de datos.

DROP TRIGGER IF EXISTS measurement_no_update;
DROP TRIGGER IF EXISTS result_no_update;

UPDATE measurement
SET unit = CASE lower(trim(unit))
    WHEN 'mm' THEN 'mm'
    WHEN 'milimetro' THEN 'mm'
    WHEN 'milimetros' THEN 'mm'
    WHEN 'milímetro' THEN 'mm'
    WHEN 'milímetros' THEN 'mm'
    WHEN 'cm' THEN 'cm'
    WHEN 'centimetros' THEN 'cm'
    WHEN 'centímetros' THEN 'cm'
    WHEN 'm' THEN 'm'
    WHEN 'mt' THEN 'm'
    WHEN 'mts' THEN 'm'
    WHEN 'metros' THEN 'm'
    WHEN 'g' THEN 'g'
    WHEN 'gr' THEN 'g'
    WHEN 'grs' THEN 'g'
    WHEN 'gramos' THEN 'g'
    WHEN 'kg' THEN 'kg'
    WHEN 'kgs' THEN 'kg'
    WHEN 'kilogramos' THEN 'kg'
    WHEN 'n' THEN 'N'
    WHEN 'kn' THEN 'kN'
    WHEN 'mpa' THEN 'MPa'
    WHEN 'pct' THEN '%'
    WHEN 'porcentaje' THEN '%'
    WHEN 'c' THEN '°C'
    WHEN '°c' THEN '°C'
    WHEN 'celsius' THEN '°C'
    WHEN 's' THEN 's'
    WHEN 'seg' THEN 's'
    WHEN 'segundos' THEN 's'
    ELSE unit
END
WHERE unit NOT IN ('mm', 'cm', 'm', 'g', 'kg', 'N', 'kN', 'MPa', '%', '°C', 's');

UPDATE result
SET unit = CASE lower(trim(unit))
    WHEN 'mm' THEN 'mm'
    WHEN 'm' THEN 'm'
    WHEN 'mts' THEN 'm'
    WHEN 'g' THEN 'g'
    WHEN 'kg' THEN 'kg'
    WHEN 'kgs' THEN 'kg'
    WHEN 'n' THEN 'N'
    WHEN 'kn' THEN 'kN'
    WHEN 'mpa' THEN 'MPa'
    WHEN 'pct' THEN '%'
    WHEN '°c' THEN '°C'
    WHEN 'c' THEN '°C'
    ELSE unit
END
WHERE unit NOT IN ('mm', 'cm', 'm', 'g', 'kg', 'N', 'kN', 'MPa', '%', '°C', 's');

CREATE TRIGGER measurement_no_update
BEFORE UPDATE ON measurement
BEGIN
    SELECT RAISE(ABORT, 'una medición no se modifica: registre una versión corregida');
END;

CREATE TRIGGER result_no_update
BEFORE UPDATE ON result
BEGIN
    SELECT RAISE(ABORT, 'un resultado calculado no se modifica: emita uno nuevo');
END;
