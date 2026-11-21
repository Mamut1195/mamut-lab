# Fase 0 · Registro de riesgos

> Probabilidad (P) e impacto (I): **B**aja · **M**edia · **A**lta · **C**rítica.
> Revisión obligatoria al cierre de cada fase.

## Los tres que pueden hundir el proyecto

| # | Riesgo | Por qué es grave |
| --- | --- | --- |
| **R-05** | No disponer de las normas oficiales (ASTM, AASHTO, NTE INEN) | Sin ellas no se puede emitir conformidad con responsabilidad. Es el bloqueante comercial real, no la tecnología. |
| **R-06** | Implementar una fórmula mal, o una cláusula mal interpretada | Un resultado incorrecto en un informe es un daño legal y de seguridad, y destruye la credibilidad del producto. |
| **R-02** | Dependencia de un proveedor único de framework | Si Rune se abandona, sin aislamiento se pierde todo el trabajo de interfaz. |

---

## Registro completo

### R-01 · Cambios de API en Rune (pre-release)

- **P:** Alta · **I:** Media · **Disparador:** una actualización rompe la compilación
- **Descripción:** Rune es `0.0.1` y publica a diario. Las firmas cambian entre revisiones.
- **Mitigación:** revisión fijada en un solo lugar; `Cargo.lock` versionado; ventanas de
  actualización planificadas (ADR-0008); pruebas de interfaz sobre claves estables.
- **Plan de contingencia:** no actualizar. La revisión fijada sigue compilando.

### R-02 · Proveedor único y sin vía de contribución

- **P:** Media · **I:** Alta · **Disparador:** proyecto sin commits > 6 meses; cambio de licencia; bloqueo funcional
- **Descripción:** Rune lo mantiene una empresa y no acepta *pull requests* (`README.md:63`). No se puede corregir un fallo del framework.
- **Mitigación:** ADR-0008 (aislamiento + disparadores de salida medibles). El dominio
  nunca importa `rune`.
- **Plan de contingencia:** portar la capa de presentación a `egui`/`iced`; estimado en
  semanas, no en un reinicio.

### R-03 · Deriva de los parches obligatorios

- **P:** Alta · **I:** Media · **Disparador:** actualizar Rune sin actualizar los cinco parches
- **Descripción:** el bloque `[patch.crates-io]` es obligatorio (hallazgo H-1). Cinco
  revisiones que deben moverse juntas; es fácil equivocarse.
- **Mitigación:** un script lee el `[patch.crates-io]` de Rune en la revisión objetivo y
  reescribe el nuestro; CI comprueba que las cinco revisiones coinciden con la de `rune`.
- **Plan de contingencia:** volver a la revisión anterior (procedimiento documentado).

### R-04 · Bloqueo de `cargo` al descargar la dependencia git en Windows

- **P:** Alta en Windows · **I:** Media · **Disparador:** un desarrollador nuevo no compila
- **Descripción:** verificado en fase 0: con `libgit2` la descarga se queda detenida.
- **Mitigación:** `CARGO_NET_GIT_FETCH_WITH_CLI = true` en `.cargo/config.toml` y en CI,
  documentado en el README de desarrollo. **Acción de fase 1.**

### R-05 · Disponibilidad y licencia de las normas

- **P:** Alta · **I:** Crítica · **Disparador:** no se pueden adquirir o no hay revisor asignado
- **Descripción:** ASTM y AASHTO son documentos con licencia; las NTE INEN tienen su
  propio régimen. Sin texto oficial no hay implementación verificable.
- **Mitigación:** adquirir el paquete mínimo de la fase 2 (C39/C39M y las de suelos de
  la fase 3); nombrar un ingeniero responsable que firme cada `NormStatus::Verified`;
  el sistema **no emite conformidad** mientras falte (ADR-0006).
- **Plan de contingencia:** el producto sigue siendo útil como registro y cálculo
  interno; se comunica la limitación en la interfaz y en el informe.

### R-06 · Error de implementación normativa

- **P:** Media · **I:** Crítica · **Disparador:** una revisión de ingeniería detecta una discrepancia
- **Descripción:** interpretar mal una cláusula (redondeo, tolerancia, número de
  especímenes, criterio de aceptación) produce resultados formalmente incorrectos.
- **Mitigación:** valores de referencia **con procedencia documentada**; revisión
  independiente por un ingeniero; fixtures de regresión que no se autogeneran;
  separación cálculo/conformidad; avisos visibles cuando la norma no está revisada.
- **Plan de contingencia:** versión de motor de cálculo + registro de auditoría permite
  identificar y recalcular todos los informes afectados.

### R-07 · Error numérico (redondeo, comparación con límites)

- **P:** Media · **I:** Alta · **Disparador:** un resultado cae justo en el límite
- **Descripción:** comparar un valor ya redondeado contra un límite es un error clásico.
- **Mitigación:** comparar con el valor sin redondear; newtypes de unidades que impiden
  mezclar dimensiones; tolerancias explícitas en pruebas; ADR-0006 §5.

### R-08 · Complejidad de la maquetación del PDF

- **P:** Media · **I:** Media · **Disparador:** las tablas del informe no fluyen bien con `printpdf`
- **Mitigación:** `printpdf` detrás de un trait; prueba de concepto al inicio de la fase 2;
  evaluar Typst como biblioteca si estorba.
- **Nota:** `printpdf 0.12.11` compila en este stack (verificado).

### R-09 · Desbordamiento de alcance en gráficos

- **P:** Media · **I:** Baja · **Disparador:** peticiones de zoom, tooltips, animaciones, 3D
- **Mitigación:** `mamut-charts` entrega geometría; alcance mínimo por fase definido en
  ADR-0004. Se añade interacción por necesidad demostrada, no por catálogo.

### R-10 · Accesibilidad real en Windows

- **P:** Media · **I:** Media · **Disparador:** una licitación pública exige lector de pantalla
- **Descripción:** los parches de `accesskit` compilan, pero UI Automation no se probó.
- **Mitigación:** activar la feature `accessibility` en fase 1 y probar con Narrador.

### R-11 · Deriva entre sistemas operativos

- **P:** Media · **I:** Baja · **Disparador:** una regresión aparece solo en macOS
- **Descripción:** el CI de macOS de Rune corre por agenda, no por cambio.
- **Mitigación:** matriz de CI propia (Windows obligatorio, Linux/macOS informativos).

### R-12 · Arranque percibido lento la primera vez

- **P:** Baja · **I:** Media · **Disparador:** usuarios reportan que «tarda en abrir»
- **Descripción:** la primera ejecución compiló shaders: **11,8 s** de inicio de GPU;
  las siguientes, **112,7 ms**. Medido en build debug.
- **Mitigación:** medir en `--release` al inicio de la fase 1; si el arranque en frío es
  malo, estudiar precompilación de shaders o pantalla de bienvenida.

### R-13 · Empaquetado, firma y actualizaciones (fase 6)

- **P:** Alta · **I:** Alta · **Disparador:** no hay certificado de firma con antelación
- **Descripción:** Rune no incluye instalador ni actualizador. La firma de código tiene
  plazos de emisión y coste anual; sin ella, SmartScreen advierte.
- **Mitigación:** iniciar la compra del certificado durante la fase 3, no en la fase 6.

### R-14 · Pérdida o corrupción de datos

- **P:** Baja · **I:** Crítica · **Disparador:** corte de luz, disco lleno, cierre forzado
- **Mitigación:** WAL; transacciones; copia de seguridad automática diaria verificada;
  `PRAGMA integrity_check` al restaurar; pruebas de migración y de restauración.
  Un laboratorio no puede perder años de ensayos.

### R-15 · Desviación de alcance hacia un ERP de construcción

- **P:** Alta · **I:** Alta · **Disparador:** peticiones de presupuestos, cubicaciones, BIM, facturación
- **Mitigación:** el dominio está escrito y acotado; toda petición fuera del alcance pasa
  por control de cambios y se responde por escrito.

### R-16 · Dependencia de una sola persona con conocimiento del dominio

- **P:** Media · **I:** Media
- **Mitigación:** ADRs, fichas normativas, documentación de cálculo y pruebas como
  documentación viva; revisión cruzada obligatoria en los ensayos.

### R-17 · Vulnerabilidades en la cadena de dependencias

- **P:** Media · **I:** Alta · **Disparador:** aviso de CVE en `wgpu`, `image`, `rfd`, etc.
- **Mitigación:** `cargo audit` en CI; actualizaciones de seguridad fuera de la ventana
  ordinaria; registro de la revisión de Rune en uso para poder reaccionar.

### R-18 · Fechas y zonas horarias en informes

- **P:** Baja · **I:** Media · **Disparador:** un informe con fecha de un día anterior
- **Mitigación:** instantes en UTC en la base; zona del laboratorio configurable
  (`America/Guayaquil` por defecto); pruebas de frontera (23:59 y cambio de día).

---

## Riesgos aceptados explícitamente

| Riesgo | Aceptado porque |
| --- | --- |
| Rune es pre-release | Aporta capacidades que ahorran meses; el riesgo se acota con ADR-0008 |
| Habrá ensayos que calculan sin emitir conformidad | Es preferible a emitir un juicio sin respaldo normativo |
| El producto no será un ERP | Es una decisión de producto, no una limitación técnica |
