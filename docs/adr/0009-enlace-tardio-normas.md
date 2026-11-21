# ADR-0009 · Enlace tardío de las normas ("norm pack")

- **Estado:** aceptado
- **Fecha:** 2026-11-20
- **Matiza:** ADR-0006 (mantiene la puerta de conformidad, elimina el bloqueo de fase)
- **Decisores:** arquitectura, ingeniería de materiales, QA

## Contexto

En la Fase 0 se identificó como bloqueante la falta de las normas oficiales
(ASTM, AASHTO, NTE INEN). El bloqueante era real, pero estaba **mal planteado**:
trataba como una sola cosa tres capas distintas con costes y riesgos muy diferentes.

La consecuencia era un proyecto detenido esperando un documento. Este ADR corrige eso
sin relajar ninguna garantía de rigor.

## Decisión

Las normas se incorporan como **datos externos versionados** (un *norm pack*), no como
código. El producto se construye completo sin ellas y las recibe al final, sin
recompilar.

### Las tres capas (esto es el núcleo de la decisión)

| Capa | Qué contiene | ¿Disponible ya? | Quién la aporta |
| --- | --- | --- | --- |
| **1. Metadatos** | Designación, año, título, cuerpo, alcance resumido | **Sí**, fuentes públicas | El pack, con URL y fecha de consulta |
| **2. Matemática** | Relaciones entre magnitudes (σ = P/A, % que pasa, PI = LL − LP, γ_d = γ/(1+w)) | **Sí**, es definición de la magnitud o literatura técnica abierta | `lab-calculations`, con pruebas verificables a mano |
| **3. Contenido normativo** | Procedimiento, velocidad de carga, tablas de corrección, tolerancias, criterios de aceptación | **No** | El documento oficial + revisión de un ingeniero |

La capa 3 es la que decide si un resultado es **aceptable**. Es exactamente la que no
se puede reconstruir de memoria con responsabilidad, y por eso no se reconstruye.

### Regla de implementación (sustituye a «no inventar fórmulas» con algo aplicable)

Una fórmula **puede** implementarse antes de tener la norma si y solo si cumple una de
estas condiciones:

1. Es la **definición** de la magnitud (tensión = fuerza / área; humedad = agua / sólido).
2. Aparece en **literatura técnica abierta** y es estable entre ediciones.

Y en ambos casos debe:

- marcarse `NormStatus::PendingReview`;
- probarse con valores **verificables a mano** (números redondos, cálculo reproducible
  en una servilleta), no con valores «que salen» del propio código;
- llevar una nota de qué detalle falta por confirmar.

Queda **prohibido** en toda circunstancia:

- Tomar de memoria una tabla de corrección, un factor, una tolerancia o un criterio de
  aceptación y presentarlo como normativo.
- Asignar `NormStatus::Verified` sin nombre de revisor y fecha.
- Emitir un juicio de conformidad cuando la especificación o la norma no están
  verificadas (esto ya lo garantizaba ADR-0006 y se mantiene intacto).

### Uso legítimo del conocimiento previo

El equipo **sí** puede redactar una propuesta de implementación a partir de su
conocimiento del método, siempre que:

- se marque como `PendingReview` y con `source: "draft"`;
- sirva para que el ingeniero revisor **valide o corrija**, no para certificar;
- el detalle dudoso se deje explícito como `TODO` en lugar de rellenarse con una
  suposición plausible.

Un borrador honesto acelera la revisión. Un dato inventado la destruye.

### Formato del pack

JSON versionado en `docs/normas/packs/`, validado por esquema. Contiene por norma:
metadatos, estado, revisor, cláusulas implementadas y pendientes, límites de
especificación (`null` mientras no existan) y notas de implementación.

**Añadir una norma verificada = editar un archivo de datos.** No hay recompilación, ni
migración de código, ni nueva versión del producto: la aplicación busca packs en
`<carpeta de datos>/norm-packs/*.json` al arrancar y usa el primero válido, con el pack
embebido como respaldo (implementado en `crates/mamut-app/src/pack.rs`).

## Consecuencias

**Positivas**

- La Fase 2 deja de estar bloqueada: se construye el ciclo completo y se prueba.
- Incorporar una norma al final es una tarea de configuración, no de desarrollo.
- El producto es vendible **antes** de tener las normas: vale por trazabilidad,
  cálculo reproducible, historial e informes.
- El cliente puede cargar sus propias normas y sus propios criterios de aceptación
  (que, en la práctica, casi siempre vienen del proyecto, no de la norma).

**Negativas**

- Durante un tiempo el producto calcula pero no dictamina. Debe comunicarse con claridad
  al usuario y al cliente; no es un defecto oculto.
- El pack inicial contiene metadatos sin contenido: hay que resistir la tentación de
  rellenarlo «para que parezca completo».
- La revisión de ingeniería sigue siendo necesaria; este ADR la aplaza, no la elimina.

## Qué cambia respecto a la Fase 0

| Antes | Ahora |
| --- | --- |
| «Fase 2 bloqueada por falta de normas» | Fase 2 avanza en **modo cálculo**; la conformidad se activa al cargar el pack verificado |
| «Necesito las normas para empezar» | Se empieza ya; las normas entran al final como datos |
| Riesgo R-05 (normas) | Sigue existiendo, pero **deja de bloquear**; pasa a ser un hito comercial |

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| Sacar las fórmulas y los criterios de memoria e implementarlos como verificados | Fraude técnico: un informe avalado con datos recordados es indefendible ante una auditoría |
| Esperar a tener todas las normas | Detiene el proyecto meses por un documento que solo afecta a la última capa |
| Hardcodear las normas en el código | Cada norma nueva sería un desarrollo y un despliegue; el enlace tardío es estrictamente mejor |
| No emitir nunca conformidad | Priva al cliente de la función que más valora cuando tiene su especificación de proyecto |
