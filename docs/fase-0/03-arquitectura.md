# Fase 0 · Arquitectura (vista de conjunto)

Complementa al ADR-0003 (que fija los crates y las reglas de dependencia). Aquí se
describe **cómo encaja todo**: modelo de datos, flujo de un ensayo y estado de la
aplicación.

---

## 1. Capas

```
┌───────────────────────────────────────────────────────────────┐
│ mamut-app        Interfaz Rune · estado de UI · composición    │  ← único que conoce Rune
├───────────────────────────────────────────────────────────────┤
│ casos de uso     (funciones en mamut-domain / mamut-storage)   │
├───────────────────────────────────────────────────────────────┤
│ dominio          muestras · especímenes · ensayos · resultados │
│                  normas · conformidad · auditoría              │
├───────────────────────────────────────────────────────────────┤
│ servicios puros  cálculos · validación · gráficos · informes   │
├───────────────────────────────────────────────────────────────┤
│ infraestructura  SQLite · archivos · reloj · exportación       │
└───────────────────────────────────────────────────────────────┘
```

La regla que sostiene todo: **las flechas de dependencia apuntan hacia abajo**.
El dominio no sabe que existe una base de datos ni una pantalla.

---

## 2. Modelo de datos

### Entidades principales

```
Cliente ──┐
Proyecto ─┼──► Muestra ──► Espécimen ──► Ensayo ──► Medición
Material ─┘        │                        │
                   │                        ├──► Resultado calculado (inmutable)
                   └──► Adjunto             ├──► Equipo usado
                                            └──► Informe (PDF versionado)
```

### Decisiones de modelado

| Decisión | Motivo |
| --- | --- |
| `Muestra` y `Espécimen` son entidades distintas | Una muestra de hormigón produce varios cilindros; cada cilindro se ensaya por separado. Sin esta separación no hay trazabilidad real. |
| `Ensayo` referencia `NormRef` **por valor** (designación + año + estado en el momento) | Un informe debe seguir diciendo qué norma se aplicó **aquel día**, aunque el registro normativo cambie después. |
| `Resultado calculado` es **inmutable y versionado** | Regla 5 del encargo: no sobrescribir sin registrar cambios. |
| `Medición` guarda el valor tecleado como texto decimal + unidad | Se conserva el dato original tal como se introdujo (regla 4). |
| `Adjunto` direccionado por hash de contenido | Dos fotos iguales no se duplican; el archivo original nunca se modifica. |
| `Auditoría` append-only | Solo `INSERT`; una prueba lo verifica. |

### Esquema inicial (fase 1, simplificado)

```sql
cliente(id, nombre, contacto, creado_en)
proyecto(id, cliente_id, nombre, ubicacion, creado_en)
material(id, tipo, descripcion)                    -- hormigón, suelo, agregado, asfalto
muestra(id, codigo UNIQUE, cliente_id, proyecto_id, material_id,
        procedencia, fecha_recepcion, responsable_id, estado, creado_en)
especimen(id, muestra_id, designacion, dimensiones_json, creado_en)
ensayo(id, especimen_id, tipo, norma_designacion, norma_anio,
       metodo_kind, estado, fecha_ensayo, equipo_id, operador_id, creado_en)
medicion(id, ensayo_id, variable, valor_texto, unidad, orden, creado_en)
resultado(id, ensayo_id, variable, valor, unidad, cifras_significativas,
          motor_version, entradas_hash, calculado_en, calculado_por)
conformidad(id, resultado_id, veredicto, especificacion_id, motivo_no_evaluado)
adjunto(id, muestra_id, nombre, mime, sha256, bytes, creado_en)
equipo(id, nombre, marca, modelo, serie, adquirido_en, calibrado_en,
       proxima_calibracion, responsable_id, estado)
auditoria(id, momento, actor_id, entidad, entidad_id, accion, detalle_json)
esquema_version(version, aplicado_en)              -- migraciones
```

Índices obligatorios desde el inicio: `muestra(codigo)`, `ensayo(especimen_id)`,
`resultado(ensayo_id)`, `auditoria(entidad, entidad_id)`, `equipo(proxima_calibracion)`.

---

## 3. Flujo de referencia: compresión de cilindros

Es la *rebanada vertical* de la fase 2. Sirve de plantilla para todos los ensayos.

```
1. Alta de muestra          → validación de campos obligatorios, código único
2. Alta de espécimen        → dimensiones, masa, edad
3. Registro del ensayo      → equipo, operador, fecha, norma
4. Captura de mediciones    → diámetro, altura, masa, carga máxima (con unidad)
5. Validación de entrada    → ¿positivos? ¿rangos plausibles? ¿coherentes entre sí?
6. Cálculo (puro)           → área, resistencia, estadísticos del conjunto
7. Conformidad              → solo si hay especificación Y norma verificada
8. Persistencia             → resultado inmutable + auditoría + hash de entradas
9. Informe                  → PDF reproducible desde el resultado guardado
```

### Frontera del cálculo

```rust
// mamut-calculations — función pura, sin I/O, sin Rune
pub fn compressive_strength(
    input: &CylinderCompressionInput,
) -> Result<CompressionOutcome, CalcError>;

pub struct CylinderCompressionInput {
    pub diameter: Length,
    pub height: Length,
    pub mass: Mass,
    pub max_load: Force,
    pub age_days: u32,
    pub norm: NormRef,
}

pub struct CompressionOutcome {
    pub area: Area,
    pub stress: Stress,               // carga / área
    pub height_diameter_ratio: f64,   // control de esbeltez
    pub warnings: Vec<CalcWarning>,   // p. ej. relación fuera de rango
}
```

Nada de esto conoce la base de datos, la pantalla ni el PDF. Se prueba con
`cargo test` en milisegundos.

### Estados de un ensayo

```
Borrador ──► Registrado ──► Calculado ──► Informado
    │             │              │
    └─────────────┴──────────────┴──► Anulado (con motivo, auditado)
```

- **Borrador:** se puede editar libremente.
- **Registrado:** los datos originales quedan fijados; corregir exige motivo.
- **Calculado:** existe un resultado inmutable con su hash de entradas.
- **Informado:** se emitió un PDF; el ensayo ya no se modifica.
- **Anulado:** no se borra nunca; se marca y se explica.

---

## 4. Estado de la aplicación (interfaz)

Rune usa un modelo explícito: `fn view(&S, &mut Cx<S>) -> El`. Por tanto:

```rust
pub struct AppState {
    pub nav: NavState,          // sección activa, historial
    pub samples: SamplesState,  // filtros, selección, borrador en edición
    pub tests: TestsState,      // ensayo abierto, mediciones en curso
    pub equipment: EquipmentState,
    pub reports: ReportsState,
    pub session: SessionState,  // usuario, laboratorio, tema, avisos
    pub toast: ToastStackState,
    pub jobs: JobsState,        // trabajos en curso (importar, PDF, backup)
}
```

Reglas:

- La vista **no muta** el estado: los manejadores (`.on_press`, `.on_input`) sí.
- Ninguna consulta a la base de datos se hace dentro de `view`. Se pide al hilo de
  almacenamiento y el resultado llega como evento que muta el estado.
- Los formularios guardan **texto** mientras se editan y se convierten a unidades
  solo al validar, para no perder lo que el usuario escribió.

---

## 5. Almacenamiento en disco

```
%APPDATA%\MamutLab\                 (config)  preferencias, tema, laboratorio
%APPDATA%\MamutLab\data\
    mamut.db                        base de datos (WAL)
    attachments\<sha256>\<nombre>   adjuntos inmutables
    reports\<anio>\<informe>.pdf    informes emitidos
    backups\mamut-AAAAMMDD-HHMM.db  copias verificadas
%LOCALAPPDATA%\MamutLab\logs\       registros rotativos
```

Se resuelve con `rune::dirs::AppDirs`, que ya contempla las convenciones de cada
sistema y permite aislar las pruebas con `RUNE_DIRS_ROOT` (verificado en fase 0).

**Copia de seguridad:** `VACUUM INTO` a un archivo nuevo + `PRAGMA integrity_check`
sobre la copia + registro del resultado. Restaurar exige verificar antes de sustituir.

---

## 6. Preparado para sincronización futura (no implementada)

- Identificadores **UUIDv7** en todas las entidades: únicos entre instalaciones y
  ordenables por tiempo, sin coordinación central.
- Marca de tiempo lógica por fila (`updated_at` + `revision`): permite resolución de
  conflictos de último escritor con detección de cambios concurrentes.
- Auditoría append-only: es exactamente el registro que necesita un motor de
  replicación.
- Los repositorios son concretos en la fase 1; se abstraerán en *traits* cuando exista un
  adaptador de sincronización, para no construir abstracción especulativa.

No se implementa nada de esto en la primera versión, pero el modelo no lo impide.

---

## 7. Rendimiento: qué vigilar desde el inicio

| Punto | Objetivo | Cómo se mide |
| --- | --- | --- |
| Arranque en frío | < 2 s percibidos | Medición en release; atención a la compilación de shaders |
| Apertura de una lista de 10.000 muestras | < 200 ms | Tabla virtualizada, consulta paginada |
| Cálculo de un ensayo | < 10 ms | Es aritmética pura |
| Generación de PDF | < 1 s | Trabajo en hilo aparte, con progreso |
| Copia de seguridad de 1 GB | < 10 s | `VACUUM INTO` en hilo aparte |

Sin estas cifras medidas no se puede afirmar que el producto «va rápido». La fase 2 debe
dejar el arnés de medición listo (en la fase 1 no existe: es deuda DT-15).
