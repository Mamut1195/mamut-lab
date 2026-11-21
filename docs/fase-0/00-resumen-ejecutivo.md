# Fase 0 · Resumen ejecutivo y propuesta de aprobación

**Proyecto:** MAMUT Lab — software de laboratorio de materiales de construcción
**Fecha:** 2026-11-18
**Alcance de esta fase:** investigación, validación de stack, arquitectura, roadmap y riesgos. **No** se ha programado el producto.

---

## 1. Qué se pedía

Investigar Rune, validar el stack, presentar una arquitectura técnica respaldada por
documentación real, un roadmap y los riesgos, **antes** de implementar.

## 2. Qué se ha hecho

1. **Investigación de Rune contra el código**, no contra resúmenes: repositorio clonado
   (99 MB, 3.446 archivos), documentación, notas de investigación internas y las
   130+ pruebas del propio framework.
2. **Validación ejecutada** en esta máquina: se construyó una aplicación real con la
   forma del producto y se comprobó que compila, se prueba y renderiza.
3. **Arquitectura** en 8 ADRs, con reglas de dependencia verificables.
4. **Roadmap** por fases con criterios de salida medibles.
5. **Registro de riesgos** con mitigación y disparadores.

## 3. Evidencia principal (no son promesas, son resultados)

| Comprobación | Resultado |
| --- | --- |
| Compilación del stack completo en Windows | ✅ `Finished dev profile in 3m 22s`, 299 crates |
| Pruebas automáticas de interfaz, datos y exportación | ✅ **5/5**, en **0,38 s** |
| Render por GPU en hardware real (DX12) | ✅ dos temas, PNG de 2360×1600 |
| SQLite embebido (sin DLL del sistema) | ✅ ida y vuelta verificada |
| Tabla técnica de 240 filas con ordenación | ✅ virtualizada por Rune |
| Gráfico vectorial propio | ✅ geometría pura, probable sin GPU |
| XLSX | ✅ archivo real escrito en disco |
| PDF (`printpdf`) | ✅ compila (API se validará en fase 2) |

Todo el detalle, incluido lo que **no** se ha validado, está en
`02-validacion-stack.md`. Es importante leer también esa lista: la credibilidad de
este informe depende de que sus límites estén escritos.

## 4. Decisiones que requieren tu aprobación

| # | Decisión | Documento |
| --- | --- | --- |
| 1 | **Adoptar Rune** en la revisión `4a2e4379`, con la interfaz aislada y plan de salida | ADR-0001, ADR-0008 |
| 2 | **SQLite con `rusqlite`** (síncrono, embebido) en lugar de SQLx | ADR-0002 |
| 3 | **Workspace de 12 crates** con reglas de dependencia verificadas en CI | ADR-0003 |
| 4 | **Gráficos propios** de geometría pura, sin librería externa | ADR-0004 |
| 5 | **PDF con `printpdf`** tras un trait, con Typst como alternativa evaluada | ADR-0005 |
| 6 | **Registro normativo con estado**: no se emite conformidad sin norma verificada | ADR-0006 |
| 7 | **Estrategia de pruebas** con valores de referencia con procedencia | ADR-0007 |

## 5. Los tres puntos que necesitan una respuesta tuya (ninguno bloquea el desarrollo)

### 5.1 Normas (ya **no** bloquea el desarrollo)

La falta de normas oficiales **no detiene el proyecto**. Se resuelve con enlace tardío
([ADR-0009](../adr/0009-enlace-tardio-normas.md)): el producto se construye completo en
**modo cálculo** y el dictamen de conformidad se activa al cargar el *norm pack*
verificado. Incorporar una norma al final es una tarea de configuración, no de
desarrollo. Explicación completa en
[`06-normas-sin-bloqueo.md`](06-normas-sin-bloqueo.md).

Lo que sí necesito de ti, **sin prisa y sin bloquear nada**:

- El nombre del **ingeniero responsable** que firmará cada revisión normativa.
- Si ya dispones de **ASTM C39/C39M** (PDF o papel), para activar el dictamen en la fase 2.
- Aclaración del régimen de uso de las **NTE INEN** en tu organización.

### 5.2 Confirmación de la plataforma inicial

La validación se hizo en **Windows**. ¿Es la plataforma objetivo del primer cliente?
Linux y macOS quedan cubiertos por la arquitectura, pero requieren su propia
verificación en la fase 1.

### 5.3 Alcance de la fase 2

Propongo que la fase 2 sea **solo compresión de cilindros de hormigón** (más muestras,
informe e historial), y dejar flexión y suelos para después. Es la forma de tener algo
realmente usable antes y de aprender del uso real.

## 6. Lo que este proyecto no va a ser

Para evitar malentendidos desde el principio: **no** será un ERP de construcción, ni
software de cálculo estructural, ni BIM, ni gestión de presupuestos. Su dominio es el
laboratorio de materiales y la trazabilidad de ensayos. El roadmap excluye
explícitamente todo lo demás (ver `04-roadmap.md`, última sección).

## 7. Siguiente paso propuesto

Si se aprueba el diseño, la **fase 1 (Fundación)** empieza con:

1. Workspace de 12 crates con reglas de dependencia y CI completo.
2. `mamut-core` con unidades, identificadores, tiempo inyectable y errores.
3. `mamut-storage` con esquema v1, migraciones y auditoría append-only.
4. Shell de la aplicación: navegación, tema, barra de estado, atajos.
5. `.cargo/config.toml` con la corrección del bloqueo de `cargo` (hallazgo H-2).

Duración estimada: **3–4 semanas**. Al final habrá una aplicación que arranca, guarda
y se prueba — sin ensayos todavía, pero con los cimientos verificados.

---

## 8. Índice de documentos

| Documento | Contenido |
| --- | --- |
| `01-investigacion-rune.md` | Qué es Rune, estado real, capacidades verificadas, comparación |
| `02-validacion-stack.md` | Evidencia ejecutada, hallazgos y límites de la validación |
| `03-arquitectura.md` | Vista de conjunto, modelo de datos, flujos |
| `04-roadmap.md` | Fases, entregables, criterios de salida, esfuerzo |
| `05-riesgos.md` | Registro de riesgos con mitigaciones |
| `06-normas-sin-bloqueo.md` | Cómo avanzar sin las normas y cómo entran al final |
| `../adr/0001…0009` | Decisiones de arquitectura |
| `../normas/README.md` | Formato del *norm pack* y flujo de revisión |
| `../deuda-tecnica.md` | Deuda asumida, con motivo y condición de pago |
