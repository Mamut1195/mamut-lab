# CBR — cuidado: lo que se encontró **no es CBR**

- **Fuente analizada:** Florida DOT, *FM 5-515 — Limerock Bearing Ratio (LBR)*, **enero 2017**.
  <https://fdotwww.blob.core.windows.net/sitefinity/docs/default-source/materials/materials/administration/resources/library/publications/fstm/methods/fm5-515.pdf>
- **Equivalencia de referencia que se necesita:** ASTM D1883 / AASHTO T 193.
- **Estado:** ⚠️ **no utilizable como CBR.** Sirve como esqueleto de procedimiento y nada más.

## Qué publica el documento

El método de Florida calcula el **LBR** (Limerock Bearing Ratio), no el CBR:

```
LBR (%) = (carga unitaria corregida a 0,1 in / 800 psi) × 100
```

donde 800 psi es la resistencia de referencia de la caliza de Florida.

## Por qué no vale para CBR

El CBR se define contra **otras cargas de referencia**:

```
CBR (0,1 in) = carga unitaria a 0,1 in / 1000 psi × 100
CBR (0,2 in) = carga unitaria a 0,2 in / 1500 psi × 100
```

y se informa el **mayor** de los dos. Con la referencia de 800 psi el número sale distinto:
un mismo suelo daría un LBR más alto que su CBR.

> La definición de CBR con 1000 y 1500 psi es la estándar del ensayo, pero se marca aquí como
> **pendiente de confirmar en D1883 / T 193**, porque es precisamente el dato que hay que
> verificar contra la norma antes de implementarlo.

Implementar LBR y llamarlo CBR sería un error de ingeniería con consecuencias directas en el
diseño de pavimentos. No se hace.

## Lo que sí se puede reutilizar

El resto del procedimiento es común y está descrito con detalle:

1. Preparación y compactación del espécimen en molde CBR.
2. Sobrecarga (surcharge) y saturación, con medida de hinchamiento.
3. Penetración a velocidad controlada, registrando carga y penetración.
4. **Corrección de la curva carga–penetración:** si la parte inicial es cóncava hacia arriba,
   se traza la tangente en el punto de máxima pendiente y se desplaza el origen a donde esa
   tangente corta el eje horizontal.
5. Lectura de la carga corregida a 0,1 in y a 0,2 in.

Ese esqueleto es trasladable; **el divisor, no**.

## Qué hace falta para implementarlo

| Opción | Coste | Cuándo |
| --- | --- | --- |
| Comprar **D1883** o **T 193** | una norma | cuando empiece el ensayo |
| Que el primer cliente aporte su copia y su ingeniero verifique | 0 | cuando haya piloto |
| Localizar un método estatal de CBR completo (no LBR) | 0 | si se encuentra una publicación oficial; habría que verificarla igualmente |

Ninguna de las tres bloquea el resto de la fase 3: granulometría, límites y compactación ya
tienen procedimiento público completo.
