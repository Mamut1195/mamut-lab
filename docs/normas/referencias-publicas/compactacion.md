# Compactación (Proctor estándar y modificado)

- **Fuentes:**
  - Texas DOT, *Tex-113-E — Moisture-Density Curve for Base, Coarse and Treated Materials*,
    edición **enero 2025**.
    <https://ftp.dot.state.tx.us/pub/txdot-info/cst/TMS/100-E_series/pdfs/soi113.pdf>
  - Texas DOT, *Tex-114-E — Laboratory Compaction Characteristics and Moisture-Density
    Relationship of Subgrade, Embankment Soils and Backfill Material*.
    <https://ftp.dot.state.tx.us/pub/txdot-info/cst/TMS/100-E_series/archives/114-0205.pdf>
  - Florida DOT, *FM 1-T 180* (modificaciones estatales a AASHTO T 180), **mayo 2017**.
    <https://fdotwww.blob.core.windows.net/sitefinity/docs/default-source/materials/materials/administration/resources/library/publications/fstm/methods/fm1-t180.pdf>
- **Equivalencia de referencia:** ASTM D698 (estándar) / D1557 (modificado); AASHTO T 99 y T 180.
- **Estado:** implementable. Las tres ecuaciones están publicadas literalmente.

## Ecuaciones (Tex-113-E, §8.5–8.7)

**Densidad húmeda del molde:**

```
γ_húmeda = (W_T − W_M) / V_M
```

- `W_T` = masa del molde con la muestra compactada
- `W_M` = masa del molde vacío
- `V_M` = volumen del molde

**Humedad de la muestra compactada:**

```
w (%) = 100 × (W_W − W_D) / W_D
```

- `W_W` = masa húmeda de la muestra
- `W_D` = masa seca en estufa de la muestra moldeada

**Densidad seca:**

```
γ_seca = 100 × γ_húmeda / (100 + w)
```

## Redondeo publicado

- Densidad seca máxima: **0,1 lb/ft³** (1 kg/m³)
- Humedad óptima: **0,1 %**

Estas cifras son del método estatal. La norma de referencia puede fijar otras; se confirman
al revisar.

## Interpretación de la curva

La curva humedad–densidad seca tiene un máximo. El producto debe informar:

- densidad seca máxima (`γ_d,max`),
- humedad óptima (`w_opt`),
- y **cuántos puntos** sostienen el máximo, porque con tres puntos dispersos el máximo no es
  un resultado, es una interpolación.

Esto último no lo exige ninguna norma: es una decisión de honestidad del producto. Un
máximo obtenido con dos puntos no debería presentarse igual que uno con cinco.

## Diferencia entre estándar y modificado

La diferencia está en la **energía de compactación** (masa del martillo, altura de caída,
número de capas y golpes), no en las ecuaciones. Esas constantes se registran con el ensayo y
se informan en el reporte; cambiarlas cambia el resultado. Los métodos estatales las fijan en
su procedimiento y la norma de referencia, en la suya: **hay que confirmarlas, no suponerlas**.

## Observación sobre ediciones

Tex-114-E (§8.1) remite a «las ecuaciones de Tex-113-E, Sección 9», pero en la edición de
enero 2025 de Tex-113-E las ecuaciones están en la **sección 8**. Las remisiones internas
envejecen: es un recordatorio de que cada número debe citarse por su edición.

## Qué hay que confirmar contra D698 / D1557

| Punto | Por qué |
| --- | --- |
| Energía, masa del martillo, número de capas y golpes | Define si el ensayo es estándar o modificado |
| Corrección por partículas gruesas (over-size) | D698/D1557 publican la fórmula de corrección; sin ella el resultado no es comparable |
| Número mínimo de puntos de la curva | Afecta a la validez del máximo |
| Reutilización de material | Los métodos estatales la prohíben; la norma lo precisa |
