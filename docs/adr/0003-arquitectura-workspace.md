# ADR-0003 · Arquitectura del workspace y separación de responsabilidades

- **Estado:** aceptado
- **Fecha:** 2026-11-18

## Contexto

El requisito es una arquitectura modular con separación estricta de
responsabilidades, algoritmos de cálculo **nunca** mezclados con componentes
visuales, y trazabilidad completa. La estructura propuesta en el encargo parte de
`lab-core`, `lab-domain`, `lab-storage`, `lab-validation`, `lab-calculations`,
`lab-reports`, `lab-equipment`, `lab-import`, `lab-export`, `lab-app`.

## Decisión

Se adopta una arquitectura **en capas con dependencias dirigidas**: el dominio no conoce
la infraestructura ni la interfaz. La inversión de dependencias completa (*traits* de
repositorio) se introducirá cuando exista un segundo adaptador —la sincronización futura—;
hasta entonces sería abstracción especulativa (DT-14).
Se añaden tres crates a la propuesta original y se renombra el prefijo a `mamut-`.

```
mamut-lab/
  Cargo.toml                  # workspace + revisión única de Rune + [patch.crates-io]
  crates/
    mamut-core/               # tipos base: Ids, tiempo, unidades, errores, auditoría
    mamut-standards/          # (NUEVO) registro normativo: ASTM/AASHTO/NTE INEN, versiones, estado
    mamut-domain/             # entidades e invariantes: muestra, espécimen, ensayo, resultado
    mamut-calculations/       # algoritmos puros; un módulo por ensayo; sin I/O
    mamut-validation/         # validación de entrada y conformidad con especificación
    mamut-charts/             # (NUEVO) geometría de gráficos: escalas, ticks, series; sin UI
    mamut-storage/            # SQLite: esquema, migraciones, repositorios
    mamut-reports/            # composición de informes + PDF
    mamut-equipment/          # instrumentos, calibraciones, mantenimiento
    mamut-import/             # importación (CSV/XLSX, equipos, datos de campo)
    mamut-export/             # exportación (CSV/XLSX/JSON)
    mamut-app/                # binario: interfaz Rune + composición de dependencias
  tests/                      # (fase 2) pruebas de extremo a extremo
  fixtures/                   # (fase 2) datos de referencia y casos normativos
  docs/                       # ADRs, roadmap, riesgos, deuda técnica, manuales
```

### Dirección de dependencias (regla dura, verificada en CI)

```
mamut-app ──► mamut-reports, mamut-export, mamut-import, mamut-storage, mamut-equipment,
              mamut-validation, mamut-charts, mamut-domain, mamut-standards, mamut-core
mamut-storage ──► mamut-domain, mamut-core            (implementa los repositorios)
mamut-reports ──► mamut-domain, mamut-standards, mamut-core
mamut-calculations ──► mamut-domain(1), mamut-core
mamut-domain ──► mamut-standards, mamut-core
mamut-charts ──► (nada)
```

Reglas que **fallan la compilación** o el CI si se rompen:

1. `rune` **solo** puede aparecer en `mamut-app` (y en ningún otro crate).
2. `mamut-calculations` **no** puede depender de `mamut-storage` ni de `mamut-app`.
3. `mamut-domain`, `mamut-calculations`, `mamut-validation` y `mamut-standards` **no**
   pueden hacer I/O: ni archivos, ni reloj del sistema (se inyecta), ni red.
4. Ningún crate usa `unwrap()`/`expect()` sobre datos de entrada del usuario.

Se verifica con pruebas de arquitectura en CI (`crates/mamut-app/tests/architecture.rs`),
que inspeccionan los manifiestos de los doce crates. La comprobación de que el dominio no
hace I/O es **revisión de código**, no una prueba automática: está declarada como deuda.

### El ensayo como módulo autónomo

El punto crítico del producto: **cada ensayo tiene su propio modelo de entrada,
validaciones, cálculos y resultados, y no comparte algoritmo con los demás.**

```rust
// mamut-standards
pub struct TestDescriptor {
    pub code: TestCode,            // p.ej. "CONC-COMP-CYL"
    pub name: &'static str,        // "Compresión de cilindros de hormigón"
    pub norm: NormRef,             // designación + año + cuerpo
    pub method_kind: MethodKind,   // Normative | Internal
    pub status: NormStatus,        // Verified | PendingReview | PendingMetadata
}

// mamut-calculations: funciones puras, sin estado global y sin I/O
pub fn compressive_strength(input: &CylinderCompressionInput)
    -> Result<CompressionOutcome, CalcError>;
```

- La UI **no calcula**: llama a `mamut-calculations` y presenta el resultado.
- El resultado calculado incluye **qué se usó**: entradas, unidad de cada una,
  versión del motor de cálculo y referencia normativa.
- Añadir un ensayo (flexión, Marshall, CBR) es añadir un módulo + su descriptor +
  sus pruebas + su plantilla de informe. No se toca el código de los demás.

### Modelo de hilos

- **Hilo de interfaz**: Rune, vista pura del estado. Nunca bloquea.
- **Hilo de almacenamiento**: un único propietario de la conexión de escritura.
  La UI le envía comandos por canal y recibe eventos.
- **Trabajos largos** (importación, generación de PDF, copia de seguridad): hilo
  propio; la UI muestra progreso y el resultado llega como evento.
- El estado de la aplicación vive en **una sola estructura** (`AppState`), mutada
  solo por manejadores; es lo que hace la interfaz predecible y probable.

### Manejo de errores

- Un tipo de error por crate con `thiserror`; `mamut-core::Error` para la frontera.
- **Prohibido** `panic!`/`unwrap()` en rutas alcanzables por datos del usuario.
- Los errores se muestran al usuario en lenguaje de laboratorio («la masa debe ser
  mayor que cero»), no como mensajes técnicos.
- Un cálculo que no puede hacerse devuelve `Err`, **nunca** un resultado parcial
  presentado como válido.

## Consecuencias

**Positivas**

- El dominio se prueba sin interfaz, sin base de datos y sin GPU: rápido y estable.
- Sustituir Rune (ADR-0008) toca un solo crate.
- Los ensayos crecen en paralelo sin conflicto entre módulos.
- La trazabilidad se puede auditar: el resultado guarda con qué se calculó.

**Negativas**

- Más crates y más ceremonia inicial que un único binario.
- La regla «sin I/O en el dominio» obliga a inyectar reloj e identificadores: algo
  más de código de infraestructura.
- El estado único de la UI puede crecer; se mitiga dividiéndolo en subestados por
  módulo (`SamplesState`, `TestsState`, `ReportsState`).

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| Un solo crate con módulos | Las reglas de dependencia no se pueden verificar; el dominio acaba importando la UI |
| Clean Architecture con capas `application`/`infrastructure` separadas | Ceremonia adicional sin beneficio claro para el tamaño del producto |
| Arquitectura de plugins dinámicos (`.dll`) para ensayos | Complejidad de ABI y distribución; los ensayos se añaden compilando |
