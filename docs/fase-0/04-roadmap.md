# Roadmap técnico · MAMUT Lab

> **Estado de partida:** Fase 0 cerrada (investigación, arquitectura, validación de stack).
> Las estimaciones son **rangos** para un equipo de 1–2 personas senior y deben
> recalibrarse al final de cada fase con los datos reales. No son compromisos.

## Principios de secuenciación

1. **Primero lo que se puede probar.** Cada fase termina con algo ejecutable y
   verificable, no con «estructura preparada».
2. **El dominio antes que la interfaz.** Un ensayo se implementa y se prueba en
   `mamut-calculations` antes de tener pantalla.
3. **La trazabilidad no se añade después.** Auditoría, versionado de resultados y
   registro normativo existen desde la fase 1.
4. **Nada de datos ficticios presentados como resultados.** Los datos de demostración
   están marcados como tales en la interfaz y en los informes.

---

## Fase 0 · Investigación y arquitectura — **CERRADA**

**Entregado**

- Investigación de Rune contra el código fuente (no contra resúmenes).
- Validación del stack en esta máquina: compilación y ejecución headless.
- 8 ADRs, roadmap, registro de riesgos, política normativa.
- Aplicación de prueba (`spikes/rune-probe`) que demuestra el stack completo.

**Criterio de salida cumplido:** existe una decisión arquitectónica respaldada por
evidencia ejecutada y una aplicación que compila y se prueba en Windows.

---

## Fase 1 · Fundación — **CERRADA** (2026-11-20)

Acta con evidencia y resultado de la revisión adversarial:
[`docs/fase-1/00-cierre-fase-1.md`](../fase-1/00-cierre-fase-1.md).

Se entregó lo comprometido (12 crates, dominio, normas, persistencia con auditoría
inmutable, cálculos de compresión, armazón con pruebas headless, CI y documentación) y se
corrigieron 3 hallazgos bloqueantes y 8 graves de la revisión independiente. La deuda
restante está en `docs/deuda-tecnica.md` (DT-12…DT-15).

**Objetivo original:** una aplicación vacía pero *de verdad*: arranca, guarda, migra, se prueba.

| Entregable | Criterio de aceptación |
| --- | --- |
| Workspace con los 12 crates y reglas de dependencia | Prueba de arquitectura en CI que falla si `rune` aparece fuera de `mamut-app` |
| `mamut-core`: Ids, tiempo inyectable, unidades con newtypes, errores | Unidades con ida y vuelta exacta probada; reloj falso en pruebas |
| `mamut-standards`: registro normativo + **cargador de *norm pack*** (JSON, ADR-0009) | Un pack que declare `Verified` sin revisor se rechaza; `PendingReview` impide emitir conformidad (ambos probados) |
| `mamut-storage`: esquema v1 + migraciones + 3 repositorios | Migración desde cero y desde v1 probada; auditoría append-only verificada |
| Shell de la aplicación: navegación lateral, tema claro/oscuro, barra de estado | Prueba headless: los paneles existen, el tema cambia, el atajo abre el panel |
| CI completo (fmt, clippy, test, matriz de 3 SO) | Pipeline verde y reproducible |
| Manual de desarrollo + plantilla de PR + política de deuda técnica | Documento revisado |

**Riesgo principal:** subestimar el coste del esquema de auditoría y migraciones.

---

## Fase 2 · Primer producto funcional — 5–7 semanas · **EN CURSO**

**Incremento 1 completado (2026-11-21):** alta de muestras de punta a punta, con validación
de dominio, persistencia, auditoría y contadores reales. Acta en
[`docs/fase-2/00-incremento-1.md`](../fase-2/00-incremento-1.md).

**Objetivo:** un laboratorio puede registrar una muestra, ensayar un cilindro,
obtener el resultado y emitir el informe.

| Entregable | Criterio de aceptación |
| --- | --- |
| Dashboard con indicadores reales | Cada cifra viene de la base de datos, no de constantes |
| Gestión de muestras: alta, identificación única, procedencia, cliente, proyecto, material, recepción, responsable | Identificador único garantizado por la base de datos; adjuntos con hash; historial visible |
| Adjuntos (fotos y documentos) | Copia en almacén direccionado por contenido; original intacto |
| Ensayo de compresión de cilindros | Cálculo aislado con valores de referencia documentados y casos límite |
| Registro de dimensiones, masa, carga máxima y resistencia | Datos originales inmutables; corrección versionada con motivo |
| Comparación con especificación | Dos modos: **cálculo** (informa, no dictamina) y **conformidad** (solo con norma `Verified` y especificación aplicable) |
| Historial de ensayos por muestra | Trazabilidad completa: de la muestra al cálculo y al informe |
| Informe PDF con identidad visual configurable | Reproducible; incluye norma, versión, datos originales, cálculos, gráfico, responsable |
| Gráfico de resistencia vs edad con banda de especificación | Geometría probada sin ventana; mismo gráfico en pantalla y PDF |
| Exportación CSV/XLSX | Ida y vuelta sin pérdida en las columnas exportadas |

**Criterio de salida:** un usuario real completa el ciclo muestra → ensayo → informe
sin ayuda, y las pruebas de extremo a extremo lo demuestran.

**Sobre las normas (ADR-0009):** la fase **no está bloqueada**. Se construye el ciclo
completo en **modo cálculo**: registra, calcula, guarda e informa, sin emitir juicio de
conformidad. El dictamen se activa cuando se cargue el *norm pack* verificado, y eso es
configuración, no desarrollo. Ver `06-normas-sin-bloqueo.md`.

---

## Fase 3 · Suelos — 6–8 semanas

Granulometría (D6913/D2487), límites líquido y plástico (D4318), índice de
plasticidad, clasificación SUCS y AASHTO (D2487/D3282), Proctor estándar y modificado
(D698/D1557), CBR (D1883), densidad de campo (D6938/D1556 según método).

Cada ensayo: modelo de entrada propio, validaciones propias, cálculo propio, pruebas
propias, gráfico propio (granulometría semilog), sección de informe propia.

**Criterio de salida:** cada ensayo tiene valores de referencia con procedencia y sus
casos límite; la clasificación SUCS/AASHTO se contrasta con casos resueltos publicados.

---

## Fase 4 · Agregados — 3–4 semanas

Análisis granulométrico (C136), módulo de finura, peso unitario (C29), gravedad
específica y absorción (C127/C128), contenido de humedad (C566).

**Criterio de salida:** los cálculos comparten infraestructura de tamices ya probada en
suelos, pero mantienen modelos y validaciones independientes.

---

## Fase 5 · Equipos y trazabilidad — 3–4 semanas

Inventario de equipos, certificados de calibración adjuntos, fecha de próxima
calibración, alertas locales de vencimiento, historial de mantenimiento, auditoría
completa navegable, y **vinculación ensayo ↔ equipo usado** (un resultado debe poder
responder «con qué máquina se midió»).

**Criterio de salida:** un ensayo no se puede cerrar sin equipo asignado, o se marca
explícitamente como no instrumentado; las alertas aparecen en el dashboard.

---

## Fase 6 · Producto comercial — 5–7 semanas

| Entregable | Nota |
| --- | --- |
| Instalador Windows (MSI o NSIS) con firma de código | Requiere certificado de firma; sin él, SmartScreen advierte |
| Actualizaciones | Mecanismo propio; Rune no lo incluye |
| Copia de seguridad y restauración verificadas | `VACUUM INTO` + verificación de integridad al restaurar |
| Licenciamiento offline | Clave firmada con datos de máquina; sin conexión |
| Documentación de usuario y manual técnico | En español; incluye avisos de conformidad |
| Pruebas multiplataforma | Windows prioritario; Linux/macOS según mercado |
| Paquete de normas revisadas | Fichas por norma con estado `Verified` |

**Criterio de salida:** instalación limpia en una máquina sin Rust, restauración de
una copia de seguridad y emisión de un informe en menos de una hora.

---

## Resumen de esfuerzo (estimación, 1–2 seniors)

| Fase | Rango | Acumulado |
| --- | --- | --- |
| 0 | cerrada | — |
| 1 | 3–4 semanas | 3–4 |
| 2 | 5–7 semanas | 8–11 |
| 3 | 6–8 semanas | 14–19 |
| 4 | 3–4 semanas | 17–23 |
| 5 | 3–4 semanas | 20–27 |
| 6 | 5–7 semanas | 25–34 |

**≈ 6–8 meses** hasta producto comercializable, con un alcance de ensayos acotado y
sin imprevistos mayores. Los tres riesgos que más pueden mover esta cifra están en
`05-riesgos.md` (madurez de Rune, disponibilidad de normas, alcance del PDF).

---

## Lo que este roadmap deliberadamente no incluye

- Sincronización entre equipos (la arquitectura la permite, no se implementa).
- Firma digital de informes (se prepara el dato, no la firma).
- Portal web para clientes.
- Gestión documental tipo ERP.
- Cubicaciones, presupuestos o BIM: fuera del dominio del producto.
