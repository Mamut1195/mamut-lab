# ADR-0007 · Estrategia de pruebas y reproducibilidad numérica

- **Estado:** aceptado
- **Fecha:** 2026-11-18

## Contexto

El requisito es explícito: exactitud numérica por encima de la estética, cálculos
reproducibles, sin resultados silenciosamente incorrectos, con pruebas de regresión
numérica y CI ejecutándolas.

Un fallo numérico en un laboratorio no es un bug: es un informe mal emitido, con
consecuencias legales y de seguridad estructural.

## Decisión

### Pirámide de pruebas

| Nivel | Qué se prueba | Herramienta | Cantidad esperada |
| --- | --- | --- | --- |
| Unitarias de cálculo | Cada fórmula con valores de referencia y casos límite | `cargo test` | Muchas |
| Propiedades | Invariantes (monotonía, ida y vuelta de unidades, suma de porcentajes) | `proptest` (**fase 2**) | Media |
| Contratos de dominio | Invariantes de entidades (no se puede crear un espécimen sin dimensiones positivas) | `cargo test` | Media |
| Persistencia | Repositorios contra `:memory:`, migraciones, restricciones | `cargo test` | Media |
| Importación/exportación | Ida y vuelta CSV/XLSX sin pérdida, ficheros corruptos | `cargo test` | Media |
| Interfaz | Comportamiento observable sin ventana | `rune::Runtime` | Pocas, pero reales |
| Regresión numérica | Conjunto dorado de casos con salida fijada | `cargo test` + `fixtures/` | Creciente |

### Reglas de los tests de cálculo

1. **Ningún valor de referencia sin procedencia.** Cada constante esperada se
   documenta con su origen: cláusula de la norma, ejemplo resuelto publicado, o
   cálculo manual reproducible que se adjunta. Un número «que sale» no es un test.
2. **Tolerancia explícita y justificada.** Nunca `assert_eq!` sobre un `f64` calculado.
   Se usa `assert_close(actual, expected, tolerance)` con la tolerancia razonada (absoluta
   para valores pequeños, relativa para grandes). La única excepción son valores
   **exactamente representables** y resultados de operaciones de redondeo, donde la
   igualdad es la comprobación correcta y está comentada como tal.
3. **Casos límite obligatorios por ensayo**: cero, negativo, `NaN`, infinito,
   división por cero, área nula, carga menor que la previa, edades atípicas,
   porcentajes que no suman 100, tamices desordenados.
4. **Casos de rechazo**: el ensayo debe **fallar** con entrada inválida, y el test
   comprueba el error concreto, no solo que haya error.
5. **Un test por regla normativa**, nombrado con la cláusula cuando se conoce
   (`c39_strength_is_load_over_area`, `c39_rejects_area_zero`).

### Regresión numérica

- `fixtures/regression/` (**fase 2**) contendrá casos completos (entrada + salida
  esperada) en JSON versionado. En la fase 1 los valores de referencia viven en las
  propias pruebas, con su procedencia comentada.
- La prueba recorre todos los ficheros y compara con tolerancia; si un cambio de
  algoritmo altera un resultado, la prueba falla y obliga a una decisión explícita:
  o el cambio es correcto y se actualiza el fixture **en un commit que lo justifica**,
  o es un error.
- Los fixtures **no se generan automáticamente desde el código actual** en CI: eso
  convertiría la prueba en una tautología.

### Pruebas de interfaz (lo que Rune permite)

Verificado en Fase 0 (ver `02-validacion-stack.md`):

- `Runtime::new(estado, vista, tamaño, env)` + `rt.frame(dt)` con **reloj virtual**.
- `rt.find("clave")` / `rt.presented("clave")` para localizar elementos y afirmar
  su presencia y geometría.
- `rt.pointer_down/up(x, y)` para simular clics y teclado.
- Modos `--snap`, `--tree`, `--script` para capturas y trazas deterministas.

Se prueban **comportamientos**, no píxeles: que un formulario rechaza un valor, que
la tabla muestra la fila nueva, que el botón cambia de estado. Las capturas PNG se
archivan como evidencia de revisión visual, **no** como criterio de fallo automático
al principio (los snapshots de píxeles son frágiles entre versiones del framework).

### CI

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release        (una vez por semana: detecta diferencias de optimización)
prueba de arquitectura (dependencias prohibidas)
```

Matriz: `windows-latest` (obligatorio), `ubuntu-latest`, `macos-latest`.
Windows es el objetivo; Linux y macOS avisan temprano de dependencias de sistema.

**Restricción de red:** las pruebas no acceden a Internet. Toda prueba que necesite
datos usa `fixtures/`. Si una prueba requiere red, está mal diseñada.

### Reproducibilidad de un resultado

Dado un `CalculationRecord`, debe ser posible:

1. Recalcular y obtener el mismo valor con la misma `engine_version`.
2. Explicar cualquier diferencia si la versión cambió.
3. Reimprimir el informe idéntico.

Se prueba con un test que recalcula una muestra de registros históricos y compara.

## Consecuencias

**Positivas**

- Un cambio de fórmula no puede pasar desapercibido.
- Los errores de entrada se detectan antes de emitir un informe.
- La interfaz tiene pruebas reales sin coste de mantenimiento alto.

**Negativas**

- Escribir los valores de referencia con procedencia es lento: es el trabajo, no un
  obstáculo.
- Las pruebas de interfaz pueden romperse al actualizar Rune; por eso son pocas y
  sobre claves estables.
- `proptest` añade tiempo de ejecución: se limita el número de casos en CI.
