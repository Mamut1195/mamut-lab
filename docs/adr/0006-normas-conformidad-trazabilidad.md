# ADR-0006 · Normas, conformidad y trazabilidad de cálculos

- **Estado:** aceptado · **matizado por ADR-0009** (las normas son datos de carga tardía;
  la puerta de conformidad de este ADR se mantiene intacta)
- **Fecha:** 2026-11-18

## Contexto

El encargo impone reglas que definen la credibilidad del producto:

1. No inventar fórmulas ni procedimientos normativos.
2. Cada ensayo debe indicar la norma y su versión.
3. Diferenciar métodos normativos de métodos internos.
4. Conservar los datos originales.
5. No sobrescribir resultados sin registrar cambios.
6. Mantener trazabilidad de cálculos.
7. Registrar unidades explícitamente y evitar conversiones implícitas.
8. **No presentar un resultado como conforme si no existe especificación aplicable.**

No se dispone del texto completo de las normas. ASTM y AASHTO son documentos
**con licencia y con derechos de autor**; NTE INEN tiene régimen propio.

## Decisión

### 1. Registro normativo con estado explícito

`mamut-standards` contiene un registro de referencias normativas. **Nunca** contiene
el texto de la norma: solo metadatos y nuestras propias notas de implementación.

```rust
pub struct NormRef {
    pub body: NormBody,              // Astm | Aashto | NteInen | Iso | Internal
    pub designation: &'static str,   // "C39/C39M"
    pub year: u16,                   // 2025
    pub title: &'static str,         // solo si es de dominio público; si no, referencia
    pub clause: Option<&'static str>,// cláusula aplicada, cuando se conoce
    pub status: NormStatus,
}

pub enum NormStatus {
    /// Revisada contra el documento oficial por un ingeniero responsable.
    Verified { reviewed_by: String, reviewed_on: CivilDate },
    /// Implementada, pendiente de revisión contra el documento oficial.
    PendingReview,
    /// Se conoce el nombre pero no la edición vigente.
    PendingMetadata,
}
```

**Regla de oro:** el veredicto de conformidad solo se emite si
`status == Verified` **y** existe una especificación aplicable registrada para ese
material/proyecto. En cualquier otro caso, la aplicación:

- calcula y muestra el resultado (el cálculo puede ser correcto aunque la norma no
  esté revisada), pero
- **no** muestra «conforme» ni «no conforme», y
- muestra un aviso explícito: *«Resultado calculado. No se emite juicio de
  conformidad: la referencia normativa está pendiente de revisión»*, y
- imprime ese mismo aviso en el informe PDF.

Esto es implementable como un tipo que lo hace imposible de olvidar:

```rust
pub enum Conformity {
    Conforms { spec: SpecRef },
    Exceeds { spec: SpecRef },
    NotEvaluated { reason: NotEvaluatedReason },  // no hay especificación / norma sin revisar
}
```

No existe una variante «Conforms» sin `SpecRef`, así que el compilador impide el
error más grave del producto.

### 2. Separación método normativo / método interno

`MethodKind::Normative` exige `NormRef` con designación y año. `MethodKind::Internal`
exige un identificador de procedimiento interno documentado en `docs/metodos/`.
Los informes imprimen la distinción; un método interno **nunca** se presenta como
normativo.

### 3. Trazabilidad de cálculos

Cada ejecución de cálculo produce un registro inmutable:

```rust
pub struct CalculationRecord {
    pub id: CalculationId,
    pub test_id: TestId,
    pub engine_version: &'static str,   // versión del crate de cálculos
    pub inputs_hash: Hash,              // SHA-256 del snapshot canónico de entradas
    pub inputs_snapshot: Json,          // entradas exactas, con unidades
    pub outputs: Json,                  // resultados, con unidades y precisión
    pub norm: NormRef,
    pub computed_at: Timestamp,
    pub computed_by: UserId,
}
```

- `inputs_hash` permite detectar que un resultado no corresponde a las entradas que
  hoy se muestran. **Estado en la fase 1:** se guarda y se lee, pero **todavía no se
  verifica**; la comprobación necesita el snapshot de entradas y llega en la fase 2
  (deuda DT-12). Hasta entonces, esta promesa no está cumplida y no debe presentarse
  como tal.
- Cambiar `engine_version` sin cambiar las entradas es visible en la auditoría: se
  puede reproducir un informe antiguo y explicar por qué el número cambió.

### 4. Datos originales inmutables

- Lo que el usuario teclea se guarda **tal cual**, como cadena decimal canónica, más
  su unidad. La conversión a `f64` ocurre al calcular, no al guardar.
- Corregir una lectura **crea una nueva versión** con motivo, autor y fecha; la
  anterior se conserva y se muestra como histórica.
- La tabla de auditoría es **append-only** (solo `INSERT`), y una prueba verifica que
  ningún repositorio emite `UPDATE`/`DELETE` sobre ella.

### 5. Unidades y precisión

- Newtypes por dimensión en `mamut-core::units`: `Length`, `Mass`, `Force`, `Stress`,
  `Area`, `Percent`, `Temperature`, y el enumerado `Unit` con los símbolos admitidos. Un
  `f64` suelto no cruza una frontera de módulo de cálculo.
- **Límite conocido de la fase 1:** una `Measurement` guarda el valor como texto y la
  unidad como símbolo validado, no como magnitud tipada; la conversión ocurre al entrar
  al cálculo. Tipar la medición completa es deuda DT-13.
- Conversión solo con `convert_to(unit)` explícito; no hay `From<f64>`.
- Aritmética interna en `f64`; se documenta que es suficiente porque las tolerancias
  normativas son órdenes de magnitud mayores que el error de un `f64`.
- **Regla de redondeo:** se compara contra el límite con el valor **sin redondear** y
  se informa con las cifras significativas de la norma. Comparar un valor ya
  redondeado contra un límite es un error de ingeniería y está prohibido por diseño.
- Las pruebas de cálculo usan tolerancia explícita y valores de referencia con
  procedencia documentada (ver ADR-0007).

## Consecuencias

**Positivas**

- El producto no puede afirmar una conformidad que no puede sostener.
- Un informe emitido se puede reconstruir años después.
- La revisión normativa es un estado visible, no un supuesto oculto.

**Negativas**

- Habrá ensayos que calculen pero no evalúen conformidad hasta que se revise la
  norma: es un coste comercial asumido y debe comunicarse.
- Más tablas y más código de auditoría desde la fase 1.

## Trabajo pendiente (bloqueante para la fase 2)

1. **Adquirir las normas** ASTM C39/C39M, C127, C128, C136, D698, D1557, D1883,
   D4318, D6913, D2487, D3282 y las NTE INEN aplicables.
2. **Nombrar un ingeniero responsable** de la revisión: cada `NormStatus::Verified`
   lleva nombre y fecha.
3. **Verificar el régimen de uso de las NTE INEN** (descarga pública o licencia).
4. Registrar en `docs/normas/` una ficha por norma: designación, año, cláusulas
   implementadas, cláusulas no implementadas y motivo.
