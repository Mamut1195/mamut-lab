# Norm pack · formato y flujo de trabajo

Este directorio contiene las **normas como datos**, no como código (ADR-0009). Añadir
una norma verificada es editar un JSON: no se recompila ni se publica una versión nueva
del producto.

```
docs/normas/
  README.md                  ← este archivo
  packs/
    pack-inicial.json        ← metadatos públicos, contenido PENDIENTE
    pack-cliente-ejemplo.json  ← (futuro) normas y criterios de una empresa
  fichas/                    ← (futuro) una ficha por norma, en prosa, para el revisor
```

Además, la aplicación busca packs del usuario en
`<carpeta de datos>/norm-packs/*.json` **al arrancar**: si hay uno válido, se usa en lugar
del embebido. Por eso incorporar un pack no exige recompilar (ADR-0009).

---

## Los tres estados de una norma

| Estado | Qué significa | Qué permite la aplicación |
| --- | --- | --- |
| `PendingMetadata` | Se conoce el nombre pero no la edición vigente | Nada: no se puede asociar a un ensayo |
| `PendingReview` | Metadatos correctos; el contenido **no** está validado por un ingeniero | Calcular y emitir informe **sin** juicio de conformidad |
| `Verified` | Un ingeniero revisó el documento y firmó la implementación | Calcular **y** emitir conformidad, si hay especificación aplicable |

`Verified` exige `reviewed_by` y `reviewed_on`. El cargador **rechaza** un pack que
declare `Verified` sin esos dos campos.

---

## Flujo para incorporar una norma

```
1. Localizar la edición vigente (tienda oficial del organismo)
        ↓
2. Añadir/actualizar el bloque de metadatos en el pack
   (designación, año, título, URL, fecha de consulta) → PendingReview
        ↓
3. El ingeniero responsable revisa el documento y completa:
   - cláusulas implementadas
   - cláusulas pendientes (con motivo)
   - criterios de aceptación, si la norma los define
        ↓
4. Firma: reviewed_by + reviewed_on → Verified
        ↓
5. Se copia el pack a `<carpeta de datos>/norm-packs/` y se reinicia la aplicación
   (no hace falta recompilar)
        ↓
6. La conformidad se habilita para esos ensayos
```

**Paso 2 es barato y lo puede hacer cualquier persona.** Pasos 3–4 son el trabajo del
ingeniero y no se pueden saltar.

---

## Contrato de campos

### Norma

| Campo | Tipo | Obligatorio | Notas |
| --- | --- | --- | --- |
| `id` | string | sí | Slug estable: `astm-c39-c39m-26` |
| `body` | enum | sí | `ASTM` · `AASHTO` · `NTE_INEN` · `ISO` · `INTERNAL` |
| `designation` | string | sí | `C39/C39M` (sin año) |
| `year` | integer | sí | Año de la edición |
| `title` | string | sí | Título oficial |
| `scope_summary` | string | no | Resumen propio, **no** copia del documento |
| `status` | enum | sí | `PendingMetadata` · `PendingReview` · `Verified` |
| `metadata_source` | string (URL) | sí | Dónde se confirmó la edición |
| `metadata_checked_on` | date | sí | Fecha de la consulta |
| `metadata_confidence` | enum | sí | `official` · `secondary` |
| `reviewed_by` | string \| null | si `Verified` | Nombre del ingeniero |
| `reviewed_on` | date \| null | si `Verified` | Fecha de la firma |
| `clauses_implemented` | array | no | Cláusulas cubiertas |
| `clauses_pending` | array | no | Cláusulas no cubiertas y por qué |
| `spec_limits` | object \| null | no | **`null` mientras no haya criterio aplicable** |
| `notes` | string | no | Notas de implementación |

### Ensayo asociado

| Campo | Tipo | Notas |
| --- | --- | --- |
| `code` | string | `CONC-COMP-CYL` |
| `name` | string | Nombre en español |
| `norm_id` | string | Debe existir en el pack |
| `method_kind` | enum | `Normative` · `Internal` |
| `computation` | string | Fórmula en texto, legible y auditable |
| `conformity_enabled` | bool | `false` mientras `spec_limits` sea `null` |

---

## Reglas de integridad que el cargador debe verificar

1. `status = Verified` **implica** `reviewed_by` y `reviewed_on` no nulos.
2. `conformity_enabled = true` **implica** `spec_limits` no nulo y norma `Verified`.
3. `norm_id` de cada ensayo debe existir.
4. `year` debe ser plausible: 1900–2100 para normas; 1–999 si es un método interno
   (que usa versión, no edición).
5. Un `designation` no puede estar duplicado con el mismo `year`.
6. El pack declara `pack_version`; el cargador rechaza versiones futuras desconocidas.

Estas reglas son pruebas automatizadas, no buenas intenciones: se implementan en
`mamut-standards` (códigos `E00x`) y se prueban con packs inválidos a propósito.

---

## Sobre el copyright

El pack contiene **metadatos** (designación, año, título, URL) y **notas propias**. No
contiene texto de las normas. El título oficial y la designación son datos de
referencia bibliográfica; el cuerpo del documento no se reproduce nunca.

Si un cliente aporta sus PDFs, se guardan **fuera** del repositorio y del producto
distribuido: en su instalación, bajo su licencia. El producto solo guarda el resultado
de la revisión (quién, cuándo, qué cláusulas).
