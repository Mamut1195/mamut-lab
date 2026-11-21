# Granulometría por tamices

- **Fuente:** Texas Department of Transportation, *Tex-110-E — Sieve Analysis of Soils and
  Base Materials*, edición **abril 2024**.
  <https://ftp.dot.state.tx.us/pub/txdot-info/cst/TMS/100-E_series/pdfs/soi110.pdf>
- **Equivalencia de referencia:** ASTM D6913 / AASHTO T 27 y T 88 (la parte de hidrómetro
  está en la Parte II del mismo documento).
- **Estado:** implementable. Pendiente de verificación contra D6913 para el detalle de
  preparación y para la Parte II (hidrómetro).

## Fórmulas publicadas (sección 5, *Calculations*)

Sea `W_total` la masa seca total de la muestra.

**Tamiz mayor (el primero de la serie):**

```
% retenido = 100 × (W_mayor / W_total)
```

**Tamices menores que el mayor — retenido acumulado:**

```
% retenido acumulado = 100 × (W_suma / W_total)
```

donde `W_suma` es la masa retenida en ese tamiz **más** todas las retenidas en los tamices
mayores.

**Retenido individual:**

```
% retenido individual = 100 × (W_retenido / W_total)
```

**Porcentaje que pasa:** el documento reporta el retenido; el que pasa es su complemento:

```
% que pasa = 100 − % retenido acumulado
```

## Criterios de redondeo (publicados)

- El porcentaje retenido acumulado se informa **al número entero**.
- El material que pasa el tamiz de 200 se determina por lavado con **Tex-111-E** (no está en
  este documento).

## Qué hay que confirmar contra la norma de referencia

| Punto | Por qué |
| --- | --- |
| Masa mínima de muestra por tamaño máximo de partícula | Es un requisito de la norma, no del procedimiento estatal |
| Corrección cuando hay material retenido en el tamiz de 200 | La norma fija la fórmula exacta de corrección |
| Serie de tamices exigida | Depende de la especificación del proyecto |
| Parte II (hidrómetro) | Aquí remite a AASHTO T 88; hay que contrastar con D7928, que reemplazó a D422 |

## Nota de implementación

El cálculo es puro: entra una lista de masas retenidas y sale el porcentaje que pasa por
tamiz. Encaja en `mamut-calculations` como función sin estado, con la serie de tamices como
dato de entrada (no como constante del código, porque cambia por especificación).
