# Acta de cierre · Fase 1 (Fundación)

**Fecha:** 2026-11-20 · **Estado:** cerrada tras revisión adversarial
**Alcance:** workspace, dominio, normas, persistencia, cálculos de compresión, armazón de
la aplicación y CI. **No** incluye alta de muestras ni informes PDF (fase 2).

---

## 1. Criterio de salida de la fase

El roadmap exigía: «una aplicación vacía pero *de verdad*: arranca, guarda, migra, se
prueba». Se cumple y se demuestra más abajo con comandos ejecutados.

## 2. Entregables frente a lo comprometido

| Compromiso (roadmap) | Estado | Evidencia |
| --- | --- | --- |
| Workspace con los 12 crates y reglas de dependencia | ✅ | `cargo test -p mamut-app --test architecture` (10 pruebas); detecta dependencias renombradas y exige los 12 crates |
| `mamut-core`: Ids, tiempo inyectable, unidades, errores | ✅ | 41 pruebas; `Clock`/`FixedClock`, `Unit` cerrado, SHA-256 |
| `mamut-standards`: registro normativo con estados | ✅ | 47 pruebas; pack con 10 reglas (`E00x`), conformidad imposible sin norma revisada |
| `mamut-storage`: esquema v1 + migraciones + repositorios | ✅ | 33 + 11 pruebas; migraciones `V001`/`V002` idempotentes |
| Auditoría append-only **impuesta por la base** | ✅ | 6 disparadores; `INSERT OR REPLACE` también rechazado |
| Shell: navegación, tema, barra de estado, atajos | ✅ | 9 pruebas headless con reloj virtual; captura en `armazon-dark.png` |
| CI (fmt, clippy, test, matriz de 3 SO) | ✅ | `.github/workflows/ci.yml`, incluido `schedule` semanal |
| Manual de desarrollo, plantilla de PR, deuda técnica | ✅ | `docs/desarrollo.md`, `docs/deuda-tecnica.md` (DT-01…DT-15) |

**Extra no previsto y añadido:** `mamut-charts` (geometría pura de gráficos) y
`mamut-standards` como crate propio, que no estaban en el encargo original pero eran
necesarios para separar cálculo, conformidad y presentación.

## 3. Evidencia ejecutada

```
cargo fmt --all -- --check                       →  exit 0
cargo clippy --workspace --all-targets -- -D warnings  →  exit 0
cargo test --workspace                           →  exit 0
```

- **266 pruebas**, 28 suites, 0 fallos.
- Cobertura por capa: `core` (unidades, tiempo, ids, auditoría, huellas), `standards`
  (normas, conformidad, pack con 16 códigos de error y 2 avisos), `domain` (invariantes y
  transiciones), `calculations` (compresión y redondeo), `validation`, `charts`,
  `storage` (persistencia + 12 pruebas adversariales), `reports`, `equipment`, `import`,
  `export`, y `app` (estado, 9 pruebas de armazón y 10 de arquitectura).
- **Render real por GPU** del armazón: `docs/fase-1/armazon-dark.png` (1200×800 @2×),
  generado con `mamut-lab --snap`. Arranca con base local, pack embebido de 3 normas
  pendientes y estado vacío explicado en pantalla.
- **Esquema v3**: migraciones `V001`→`V003`, con prueba de migración desde la v1 que
  conserva datos y activa las protecciones nuevas sobre las filas existentes.

## 4. Revisión adversarial

Se encargó a un revisor independiente con contexto limpio, con instrucción explícita de
**romper** las garantías, no de validarlas. Encontró 3 bloqueantes, 8 graves y 7 menores.
Todos se clasificaron y se atendieron; ninguno se descartó sin justificación.

### 4.1 Hallazgos que eran ciertos y se corrigieron

| # | Hallazgo | Corrección | Prueba que lo fija |
| --- | --- | --- | --- |
| 1-2 | `Database::connection()` público permitía desactivar PRAGMA y borrar disparadores | `pub(crate)`; los ataques con SQL crudo se movieron a `src/adversarial.rs` | `los_pragmas_de_proteccion_estan_activos`, `un_insert_or_replace_no_puede_sobrescribir_un_resultado` |
| 1 | La inmutabilidad dependía de un PRAGMA por conexión (`INSERT OR REPLACE` la esquivaba) | `PRAGMA recursive_triggers = ON` documentado y probado | `existen_los_seis_disparadores_de_inmutabilidad` |
| 3 | Los atajos de teclado no funcionaban (`.on_key` de elemento exige foco) | Se registran en `AppSpec::on_key` / `rt.on_key` | 3 pruebas de `tests/shell.rs` |
| 4 | `fmt` y `clippy -D warnings` no pasaban | Corregido preservando la semántica de NaN (`!x.is_finite() \|\| x <= 0.0`) | gate de CI |
| 5 | Una medición se podía sobrescribir con `UPDATE` | Migración `V002` con disparadores `BEFORE UPDATE/DELETE` | `una_medicion_no_se_puede_modificar_ni_borrar` |
| 6-7 | La huella de entradas no se verifica; falta `CalculationRecord` | Se **retira la promesa** del ADR-0006 y se registra DT-12 | revisión documental |
| 8 | ADR-0009 prometía «sin recompilar» y el pack iba embebido | Carga real desde `<datos>/norm-packs/*.json` al arrancar | 5 pruebas de `pack.rs` |
| 9 | Se afirmaba arquitectura de puertos sin *traits* | Se corrige el ADR (abstracción diferida a cuando exista sincronización) y DT-14 | revisión documental |
| 10 | Un pack con código inválido cargaba y el ensayo desaparecía | `E013` (código inválido) y `E014` (duplicado) | `todos_los_ensayos_del_pack_producen_descriptor` |
| 11 | Un equipo «fuera de servicio» no se bloqueaba | `is_blocked` cubre `OutOfService` y calibración `Unknown` | 2 pruebas nuevas |
| 12 | Pruebas débiles o con nombre engañoso | `status_line()` como función pura probada; estado vacío con `empty-hint`; exportación reimportada | `la_barra_de_estado_dice_lo_que_hay`, `el_csv_exportado_se_reimporta_sin_perdida` |
| 13 | Códigos de ensayo duplicados en el pack | `E014` | `dos_ensayos_con_el_mismo_codigo_se_rechazan` |
| 14 | Una medición aceptaba `NaN`, `inf`, `1e999` | `finite_number()` en creación, corrección y lectura | `nan_e_infinito_no_son_mediciones` |
| 15 | Redondear con muchos decimales producía `NaN` | `MAX_DECIMALS = 15` y comprobación de factor | `pedir_demasiados_decimales_no_produce_nan` |
| 16 | Unidad como `String` libre | `mamut_core::Unit` cerrado; se rechaza «mts», «kgs» | `una_unidad_inventada_no_se_acepta` |
| 17-18 | Afirmaciones documentales sin respaldo; huecos menores | Documentos corregidos; `DT-12…DT-15`; CI con `schedule` | — |

### 4.2 Hallazgos que se aceptan como deuda declarada (no se ocultan)

| Deuda | Qué falta | Por qué se difiere |
| --- | --- | --- |
| DT-12 | Verificar la huella de entradas | Necesita el snapshot de entradas (fase 2) |
| DT-13 | Mediciones como magnitud tipada | Requiere `Quantity` por variable (fase 3) |
| DT-14 | Repositorios tras *traits* | Abstracción especulativa hasta que exista sincronización |
| DT-15 | `proptest`, `fixtures/regression/`, arnés de medición | Prioridad a garantías de dominio y persistencia |
| DT-16 | Los repositorios fijan el estado sin pasar por las transiciones del dominio | La máquina de estados vive en el dominio; duplicarla en SQL crearía dos verdades |

### 4.3 Segunda pasada: hallazgos nuevos y correcciones de raíz

La segunda revisión encontró que dos correcciones anteriores eran incompletas. Ambas se
cerraron en el código, no en la documentación:

| Hallazgo | Por qué era grave | Corrección | Prueba |
| --- | --- | --- | --- |
| Los invariantes se saltaban con un **literal de estructura** (`Measurement { unit: "mts", .. }`, `ResultRef { value: INFINITY, .. }`) | Un resultado infinito con unidad inventada quedaba grabado en una tabla **inmutable**: irreparable para siempre | `#[non_exhaustive]` en `Measurement` y `ResultRef` (sin literales fuera del crate) + `validate()` en lectura y escritura + **disparadores en la base** (V003) que rechazan unidad desconocida y valor no finito | `la_base_rechaza_una_unidad_inventada_y_un_valor_infinito`, `validate_detecta_un_resultado_manipulado` |
| Un pack con `name` vacío cargaba y el ensayo **desaparecía** de `descriptors()` | El usuario veía 4 ensayos en el JSON y 3 en la aplicación, sin aviso | Regla `E015` (nombre obligatorio) y `descriptors()` ya no puede descartar nada | `un_ensayo_sin_nombre_no_puede_cargarse` |
| Dos packs válidos: se usaba el primero por orden alfabético y se ignoraban los demás en silencio | Un conflicto de configuración pasaba inadvertido | Aviso `W101` con el pack elegido y los ignorados | `dos_packs_validos_avisan_de_cual_se_usa` |
| Comentarios con caracteres corruptos (U+FFFD) | Cosmético, pero delata el proceso de edición | Corregidos y verificados con un contador de reemplazos | — |
| Faltaba la prueba de migración v1→v3 que exige el roadmap | La migración funcionaba, pero nadie lo demostraba | Prueba que simula una base v1 con datos, migra y comprueba datos y protecciones | `migrar_desde_la_v1_conserva_datos_y_añade_protecciones` |

### 4.5 Tercera pasada: dos avisos menores, también cerrados

El revisor dio el veredicto **APROBADA** y dejó dos avisos menores. Se corrigieron en lugar
de anotarlos, porque ambos producían datos inalcanzables:

| Aviso | Por qué importaba | Corrección | Prueba |
| --- | --- | --- | --- |
| Una fila v1 con unidad libre («milímetros») quedaba conservada pero **ilegible** tras la v3, y al ser inmutable no había reparación | Dato atrapado: existe pero la aplicación no puede mostrarlo | Migración `V004` que normaliza los símbolos heredados (solo la forma de la unidad, nunca el valor), retirando y recreando los disparadores dentro de la transacción | `migrar_desde_la_v1_conserva_datos_y_añade_protecciones` (comprueba que «milímetros» se lee como «mm») |
| El dominio aceptaba `"MM"`/`"KN"` por comparación sin distinguir caja, pero la base los rechazaba: un valor válido para el dominio no se podía guardar | Desajuste entre dos capas que dicen lo mismo | La unidad se **normaliza al símbolo canónico** en `Measurement::new`/`from_parts` y en `ResultRef::new` | `la_unidad_se_guarda_en_su_simbolo_canonico`, `la_unidad_del_resultado_se_guarda_canonica` |

**Estado del esquema:** `V001`→`V004`, `SCHEMA_VERSION = 4`.

### 4.6 Límite de la garantía de inmutabilidad (declarado sin ambigüedad)

Los disparadores protegen contra la aplicación y contra SQL que respete el esquema. **No
protegen contra un operador con acceso al archivo** que abra la base con otra herramienta
y desactive los PRAGMA: ninguna base de datos local puede impedirlo. La defensa frente a
ese escenario es detección de manipulación (cadena de hashes verificada al abrir), que
está en DT-12 y pertenece a la fase de auditoría. Se documenta aquí para que nadie venda
una garantía que el software no puede dar.

## 5. Lo que la fase 1 **no** hace (y no finge hacer)

- No hay alta ni edición de muestras en la interfaz (fase 2).
- No hay informes PDF (fase 2): `mamut-reports` compone el contenido con un renderizador
  de texto y el contrato `ReportRenderer` para enchufar el PDF.
- No hay ensayos de suelos ni agregados (fases 3 y 4).
- Ninguna norma está `Verified`: el pack inicial solo tiene metadatos públicos.
- No hay datos de demostración en la aplicación.

## 6. Cómo reproducirlo

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p mamut-app            # abre la aplicación
```

## 7. Veredicto

**Fase 1 aprobada**, con el veredicto explícito del revisor independiente: *«APROBADA»*,
tras verificar por ejecución que los ataques que antes funcionaban ya no funcionan.

Los criterios de salida se cumplen con evidencia ejecutada; las garantías que el producto
promete están impuestas por la base de datos y cubiertas por pruebas adversariales; y lo
que no está hecho aparece como deuda declarada (DT-12…DT-17) en lugar de como una promesa
vacía.

**Siguiente:** fase 2 — alta de muestras, ensayo de compresión completo, informe PDF e
historial. Requiere, del lado del negocio, el nombre del ingeniero revisor y el pack de
normas (ADR-0009 permite cargarlo sin recompilar).
