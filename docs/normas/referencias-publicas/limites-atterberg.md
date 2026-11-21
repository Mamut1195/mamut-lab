# Límites de Atterberg

- **Fuentes:**
  - Texas DOT, *Tex-104-E — Liquid Limit of Soils and Base Materials*, edición **abril 2024**.
    <https://ftp.dot.state.tx.us/pub/txdot-info/cst/TMS/100-E_series/pdfs/soi104.pdf>
  - Texas DOT, *Tex-105-E — Plastic Limit of Soils and Base Materials*.
    <https://ftp.dot.state.tx.us/pub/txdot-info/cst/TMS/100-E_series/pdfs/soi105.pdf>
- **Equivalencia de referencia:** ASTM D4318 / AASHTO T 89 (líquido) y T 90 (plástico).
- **Estado:** implementable con una salvedad en el método de un punto (ver abajo).

## Humedad de la muestra (Tex-104-E, §8.1)

```
w (%) = (A − B) / (B − C) × 100
```

- `A` = masa del recipiente con la muestra húmeda
- `B` = masa del recipiente con la muestra seca
- `C` = masa del recipiente vacío (tara)

## Límite líquido

### Método de un punto (Tex-104-E, §8.2.1)

```
LL = W_N × (N / 25)^0.121
```

- `W_N` = humedad de la muestra en el momento del cierre de la ranura
- `N` = número de golpes con el que se cerró la ranura

El exponente **0,121** es el publicado por el método. La condición de aplicación (rango de
golpes admisible para que el método de un punto sea válido) hay que confirmarla en el texto
de la norma: el método estatal la fija en su procedimiento, pero D4318 es la referencia.

### Método multipunto

Se determina la humedad para varias tandas de golpes y se interpola la humedad
correspondiente a **25 golpes**. El método estatal publica además un factor `K` alternativo
(§8.2.2) para calcular el límite con una sola determinación.

### Redondeo

El límite líquido se informa **al número entero** de porcentaje.

## Límite plástico (Tex-105-E, §5)

```
W_agua = W_húmedo − W_seco
PL (%) = 100 × [ W_agua / (W_seco − W_tara) ]
```

Se informa **al número entero** de porcentaje.

## Índice de plasticidad

```
IP = LL − PL
```

Relación universal; el método estatal remite a **Tex-107-E** (retracción lineal) cuando se
quiere sustituir el límite líquido en materiales con comportamiento particular. Para el
producto se implementa `IP = LL − PL` y se deja la retracción como ensayo aparte si algún
día se pide.

## Qué hay que confirmar contra D4318

| Punto | Por qué importa |
| --- | --- |
| Condición de validez del método de un punto (rango de golpes) | Fuera de ese rango el resultado no es defendible |
| Número de determinaciones y criterio de repetibilidad | D4318 exige duplicados con tolerancia; sin eso no se puede rechazar un ensayo |
| Preparación de la muestra (secado, tamiz 40) | Cambia el resultado en suelos orgánicos |
| Corrección por material retenido en el tamiz 40 | Afecta al IP |

## Nota de implementación

El cálculo del límite líquido multipunto es una **regresión lineal de w frente a log₁₀(N)**.
Conviene implementarlo como función pura que devuelve también la pendiente y el coeficiente
de determinación: un ajuste con datos dispersos debe poder marcarse como dudoso, no
presentarse como un número limpio.
