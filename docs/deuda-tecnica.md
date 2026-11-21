# Deuda técnica asumida

> Regla: no se oculta deuda. Cada entrada dice **qué** se debe, **por qué** se aceptó,
> **qué riesgo** genera y **cuándo** se paga. Si una entrada no tiene condición de pago,
> no es deuda: es un defecto.

---

## DT-01 · Rune en revisión fijada de `dev` (pre-release)

- **Qué:** el producto depende de una revisión concreta de la rama `dev` de Rune, no de
  una versión publicada.
- **Por qué:** Rune no publica versiones; es la única forma de usarlo.
- **Riesgo:** no recibir correcciones sin actualizar; actualizar exige trabajo.
- **Pago:** ventana de actualización al cierre de cada fase (ADR-0008 §5). No se paga
  de una vez: es un coste recurrente planificado.
- **Dueño:** arquitectura.

## DT-02 · Cinco parches de terceros replicados en el workspace

- **Qué:** `[patch.crates-io]` con `winit`, `parley_engine`, `wgpu-hal`,
  `accesskit_windows` y `accesskit_macos`, todos a la revisión de Rune.
- **Por qué:** es obligatorio: sin `parley_engine` parcheado, la resolución de
  dependencias falla (verificado en Fase 0, hallazgo H-1).
- **Riesgo:** actualizar Rune sin mover los cinco parches rompe la compilación.
- **Pago:** automatizar la sincronización (script + comprobación en CI) en la fase 1.

## DT-03 · Gráficos propios sin interacción

- **Qué:** en la fase 2 los gráficos son estáticos (sin zoom, sin tooltips, sin
  selección de punto).
- **Por qué:** el valor está en la corrección del dato y del informe, no en la
  interacción.
- **Riesgo:** un usuario puede pedir exploración interactiva antes de tiempo.
- **Pago:** cuando exista una petición real de usuarios, no antes (ADR-0004).

## DT-04 · Maquetación de PDF a bajo nivel

- **Qué:** las tablas y los saltos de página del informe se construyen a mano sobre
  `printpdf`.
- **Por qué:** da control total y evita dependencias de sistema.
- **Riesgo:** coste alto si los informes se complican (muchas secciones variables).
- **Pago:** si la maquetación supera el 20 % del esfuerzo de la fase 2, se evalúa Typst
  como biblioteca (ADR-0005).

## DT-05 · Accesibilidad no verificada

- **Qué:** la feature `accessibility` no se activó en la prueba de Fase 0.
- **Por qué:** fuera del alcance de la validación de stack.
- **Riesgo:** una licitación pública puede exigir lector de pantalla.
- **Pago:** fase 1, activar la feature y probar con Narrador en Windows.

## DT-06 · Sin verificación en Linux ni macOS

- **Qué:** solo se ha compilado y probado en Windows.
- **Por qué:** es la plataforma objetivo inicial.
- **Riesgo:** el CI de Linux requiere dependencias de sistema (`libxkbcommon`,
  `libwayland`).
- **Pago:** fase 1, matriz de CI con los tres sistemas.

## DT-07 · Sin sincronización entre equipos

- **Qué:** cada instalación es independiente; no hay replicación.
- **Por qué:** el requisito es offline-first y no se pidió sincronización.
- **Riesgo:** un laboratorio con varias sedes necesitará compartir datos.
- **Pago:** el modelo ya lo permite (UUIDv7, auditoría append-only, repositorios tras
  traits). Se implementa solo si se vende ese caso de uso.

## DT-08 · Sin firma digital de informes

- **Qué:** el PDF no va firmado digitalmente.
- **Por qué:** requiere infraestructura de certificados y no estaba en el alcance.
- **Riesgo:** algunos clientes o acreditaciones pueden exigirlo.
- **Pago:** fase 6, aprovechando el hash canónico que ya se guarda por informe.

## DT-19 · Las escrituras ocurren en el hilo de la interfaz

- **Qué:** `Store::create_sample` escribe en SQLite desde el manejador de la interfaz.
- **Por qué:** un `INSERT` local tarda microsegundos y el usuario está esperando el
  resultado; montar un hilo con canal para eso añadiría estados intermedios sin beneficio
  perceptible.
- **Riesgo:** con una base grande, un `VACUUM` o un antivirus lento, la ventana puede
  congelarse brevemente.
- **Pago:** cuando entre la importación masiva o la generación de PDF (trabajos de segundos),
  el almacenamiento pasa a su hilo con canal, como describe ADR-0003.
- **Dueño:** arquitectura.

## DT-20 · El arranque hace E/S en el hilo principal

- **Qué:** `startup()` abre la base, aplica migraciones y carga las filas antes del primer
  frame. Rune avisa en compilación de depuración: «`init` blocked 647 ms».
- **Por qué:** es lo más simple y garantiza que la primera pantalla ya tiene datos.
- **Riesgo:** en un equipo lento o con la base en red, la ventana tarda en aparecer.
- **Pago:** fase 2, medir en release y, si el arranque en frío supera ~1 s, mostrar la
  ventana con esqueletos y cargar los datos después.
- **Dueño:** arquitectura.

## DT-18 · Auditoría de licencias del árbol de dependencias

- **Qué:** el proyecto pasa a licencia **MIT**, pero no se ha revisado la licencia de las
  528 dependencias transitivas ni se ha ejecutado `cargo audit`.
- **Por qué:** antes de distribuir un binario (no solo el código) hay que confirmar que
  todas las licencias son compatibles y que no hay vulnerabilidades conocidas.
- **Riesgo:** distribuir un binario con una dependencia GPL o con un CVE abierto.
- **Pago:** fase 6, antes del primer instalador; mientras tanto, `cargo audit` en CI.
- **Dueño:** arquitectura.

## DT-17 · Un pack del usuario puede declararse «revisado» sin que nadie lo verifique

- **Qué:** desde que el pack se carga desde `<datos>/norm-packs/`, un JSON local puede
  declarar `status: Verified` con un `reviewed_by` inventado, límites y conformidad
  habilitada. La aplicación no puede comprobar la identidad del revisor sin conexión.
- **Por qué:** el pack es un documento de configuración del laboratorio; la firma de un
  ingeniero es una declaración de la organización, no una credencial criptográfica.
- **Mitigación actual:** la procedencia del pack se muestra en la barra de estado
  (`archivo: …` / `embebido`), los packs inválidos se reportan y el origen queda visible.
- **Riesgo:** un informe podría citar como «revisada» una norma que nadie revisó, si el
  laboratorio introduce un pack con datos falsos. Es un riesgo de proceso, no de software.
- **Pago:** fase 6, con firma del pack (clave del laboratorio) si el mercado lo exige.
- **Dueño:** ingeniería de materiales.

## DT-16 · Los repositorios permiten fijar el estado sin pasar por las transiciones

- **Qué:** `SampleRepository::update_state` y `TestRecordRepository::update_state` aceptan
  un estado cualquiera. El dominio tiene la tabla de transiciones (`can_transition_to`)
  y las pruebas la cubren, pero la base no impone la máquina de estados, así que una
  llamada directa al repositorio podría saltarse una transición.
- **Por qué:** la máquina de estados difiere por entidad y por tipo de ensayo; imponerla
  en SQL duplicaría la lógica en dos sitios (peor: dos verdades que se desincronizan).
- **Mitigación actual:** solo `mamut-app` llama al repositorio, siempre después de
  `transition()`; las entidades se reconstruyen validadas al leer.
- **Riesgo:** un cambio futuro en la app que llame a `update_state` sin transición no
  fallaría en la base.
- **Pago:** fase 5, cuando el ciclo de vida del ensayo tenga más actores (equipos,
  aprobaciones) y convenga un `save(&Sample)` que persista el objeto ya validado en lugar
  de campos sueltos.
- **Dueño:** dominio.

## DT-09 · Datos de demostración

- **Qué:** la aplicación de prueba y las capturas usan datos sintéticos.
- **Por qué:** no se pueden usar resultados reales de clientes en desarrollo.
- **Riesgo:** confundir una demo con un resultado válido.
- **Pago:** la fase 1 **no** genera datos de demostración en la aplicación: la captura de
  evidencia del *spike* (`spikes/rune-probe`) usa datos sintéticos y lleva el aviso
  impreso. Si en la fase 2 se añaden datos de demostración a la app, el aviso será
  obligatorio en pantalla y en el informe.

## DT-12 · La huella de entradas no se verifica todavía

- **Qué:** `result.inputs_fingerprint` se guarda y se lee, pero ningún camino lo recalcula
  ni lo compara con las entradas actuales.
- **Por qué:** verificarlo exige persistir el *snapshot* canónico de entradas, que entra
  con el cálculo completo en la fase 2.
- **Riesgo:** la detección de «alguien editó los datos sin recalcular» que promete
  ADR-0006 **no está operativa**. Está corregido en el ADR para no venderla.
- **Límite adicional, dicho sin rodeos:** los disparadores protegen contra la aplicación y
  contra SQL que respete el esquema. Una persona con acceso al archivo y otra herramienta
  puede desactivar PRAGMA o borrar disparadores; ninguna base local puede impedirlo. La
  defensa para ese escenario es la detección de manipulación (cadena de hashes verificada
  al abrir), que es justamente lo que esta deuda cubre.
- **Pago:** fase 2, junto con `CalculationRecord`.
- **Dueño:** arquitectura.

## DT-13 · Las mediciones no son magnitudes tipadas

- **Qué:** `Measurement` guarda `value_text` + `unit` (símbolo validado contra
  `mamut_core::Unit`) y `parse_value()` devuelve `f64`. La dimensión se recupera al entrar
  al cálculo.
- **Por qué:** conservar el texto original es un requisito; tipar además la medición exige
  un `Quantity` por variable y un mapeo unidad→dimensión por ensayo.
- **Riesgo:** un `f64` sin dimensión puede cruzar una frontera si alguien lo usa mal. La
  validación de unidad reduce el riesgo pero no lo elimina.
- **Pago:** fase 3 (suelos), cuando aparezcan muchas variables con unidades distintas.
- **Dueño:** dominio.

## DT-14 · Repositorios concretos, sin *traits*

- **Qué:** `mamut-storage` expone structs sobre `&Connection`; no hay puertos en *traits*.
- **Por qué:** con un solo adaptador, la abstracción sería especulativa.
- **Riesgo:** la futura sincronización obligará a introducir la abstracción y a tocar los
  llamantes de la app.
- **Pago:** cuando se implemente sincronización (fuera del alcance actual).
- **Dueño:** arquitectura.

## DT-15 · Sin `proptest`, sin fixtures de regresión y sin arnés de medición

- **Qué:** ADR-0007 describe pruebas de propiedades, `fixtures/regression/` y medición de
  rendimiento; en la fase 1 existen valores de referencia dentro de las pruebas, pero no
  esas tres piezas.
- **Por qué:** prioridad a las garantías de dominio y persistencia.
- **Riesgo:** menos cobertura ante combinaciones raras de entrada y sin línea base de
  rendimiento.
- **Pago:** fase 2 (fixtures y propiedades) y fase 2 (medición de arranque y listados).
- **Dueño:** QA.

## DT-10 · Normas sin revisar (contenido normativo pendiente)

- **Qué:** el *norm pack* inicial solo contiene metadatos públicos; ninguna norma está
  `Verified` y no hay límites de especificación cargados.
- **Por qué:** no se dispone del texto oficial y no se reconstruye de memoria
  (ADR-0009).
- **Riesgo:** el producto calcula pero no dictamina conformidad hasta que se revise. Es
  un límite funcional visible, no un defecto oculto.
- **Pago:** por cada norma, cuando exista el documento y un ingeniero la revise. Es
  trabajo de configuración (editar el pack), no de desarrollo.
- **Dueño:** ingeniería de materiales.
- **Aviso:** esta deuda **no bloquea** ninguna fase; afecta solo a la función de
  dictamen.

## DT-11 · Metadatos normativos de fuentes secundarias

- **Qué:** las ediciones de AASHTO T 22 y NTE INEN 1573 se confirmaron en listados de
  distribuidor y citas académicas, no en la tienda oficial del organismo.
- **Por qué:** urgencia por dejar el pack inicial operativo.
- **Riesgo:** un año de edición incorrecto en el informe.
- **Pago:** confirmar contra la fuente oficial antes de marcar cualquier norma como
  `Verified` (el cargador ya rechaza `Verified` sin revisor, así que no puede colarse).
