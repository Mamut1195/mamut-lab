# ADR-0008 · Aislamiento de la interfaz y plan de salida de Rune

- **Estado:** aceptado
- **Fecha:** 2026-11-18
- **Depende de:** ADR-0001 (selección de Rune), ADR-0003 (arquitectura)

## Contexto

Rune es pre-release, de un solo proveedor y sin vía de contribución externa
(`README.md:63`: «We don't take pull requests»). El desarrollo es diario y la API
puede cambiar entre revisiones. Un producto comercial con años de vida no puede
depender de eso sin un plan explícito.

Al mismo tiempo, Rune aporta capacidades difíciles de replicar (tabla virtualizada,
shell de escritorio, arnés de pruebas determinista). Descartarlo por precaución
también sería una decisión pobre.

## Decisión

Se adopta Rune **con la interfaz tratada como un adaptador reemplazable**.

### 1. Frontera estricta

```
        ┌─────────────────────────────────────────┐
        │  mamut-app  (única capa que conoce Rune)│
        │   · vistas, claves, handlers, tema      │
        └───────────────┬─────────────────────────┘
                        │ usa
        ┌───────────────▼─────────────────────────┐
        │  dominio, cálculos, normas, validación, │
        │  almacenamiento, informes, gráficos     │
        │         (Rust puro, sin Rune)           │
        └─────────────────────────────────────────┘
```

- Solo `mamut-app` incluye `rune` en su `Cargo.toml`. Una prueba de arquitectura lo
  verifica.
- Los tipos del dominio no llevan `Color`, `El` ni nada del framework.
- La geometría de gráficos (`mamut-charts`) produce puntos y etiquetas neutros; el
  adaptador a `Shape` vive en `mamut-app`.
- Las plantillas de informe reciben datos del dominio, no del estado de la interfaz.

### 2. Métricas de acoplamiento

Al cierre de cada fase se mide:

- Número de archivos de `mamut-app` que importan `rune` (esperado: todos, es su trabajo).
- Número de archivos **fuera** de `mamut-app` que importan `rune` (**debe ser 0**).
- Líneas de `mamut-app` frente a líneas de dominio (si la interfaz crece más que el
  dominio, hay lógica de negocio en el lugar equivocado).

### 3. Disparadores de salida

Se activa la sustitución del framework si ocurre **cualquiera** de estos:

| Disparador | Umbral |
| --- | --- |
| El proyecto deja de recibir commits | > 6 meses sin actividad |
| Una actualización necesaria rompe la compilación sin solución en el repositorio | > 2 semanas bloqueado |
| Cambio de licencia | Cualquiera que deje de ser compatible con producto propietario |
| Bloqueo funcional insalvable | Una necesidad del producto imposible de implementar y sin alternativa en el framework |
| Coste de actualización | Supera el 20 % del esfuerzo de una fase, de forma sostenida |

### 4. Presupuesto de migración (estimación)

Sustituir la capa de presentación por `egui` o `iced` supondría reescribir:

- Vistas y navegación: la mayor parte de `mamut-app`.
- El adaptador de gráficos: ~30 líneas por gráfico.
- Las pruebas de interfaz: se reescriben con el arnés del nuevo framework.

**No** supondría tocar: dominio, cálculos, normas, validación, almacenamiento,
informes ni exportación — es decir, **la mayor parte del valor del producto**.

Estimación: unas pocas semanas de trabajo de presentación, no un reinicio.

### 5. Reglas de actualización de Rune

1. La revisión vive en **un solo archivo** (`Cargo.toml` del workspace) y en
   `Cargo.lock` versionado.
2. Actualizar = rama propia + `cargo update` de los cinco parches a la **misma**
   revisión + `cargo test --workspace` + revisión visual de las pantallas clave.
3. Se actualiza **por ventanas planificadas** (una por fase), nunca en medio de una
   funcionalidad a medias.
4. Si una actualización falla, se queda la revisión anterior y se documenta el motivo
   en `docs/deuda-tecnica.md`.

## Consecuencias

**Positivas**

- Se aprovecha Rune sin quedar cautivos.
- El coste de una salida es acotado y conocido de antemano.
- Obliga a mantener el dominio limpio, que es bueno por sí mismo.

**Negativas**

- Cierta duplicación de tipos en la frontera (el estado de la interfaz no es el del
  dominio; hay que mapear).
- Disciplina constante: es fácil que un tipo de Rune se filtre al dominio.

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| Confiar en Rune sin aislamiento | Aceptable para una herramienta interna, no para un producto comercial |
| Descartar Rune por precaución | Renunciar a capacidades valiosas por un riesgo que se puede acotar |
| Escribir una capa de abstracción de UI genérica sobre varios frameworks | Complejidad enorme para un beneficio hipotético; la frontera en el dominio basta |
