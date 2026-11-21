# Fase 3 · Inicio — Núcleo de cálculo de suelos

**Fecha:** 2026-11-21 · **Estado:** en curso (incremento 1)

## Qué se ha implementado

Tres módulos de cálculo puro en `mamut-calculations`, con sus ecuaciones tomadas de las
referencias públicas documentadas en `docs/normas/referencias-publicas/`:

| Módulo | Qué calcula | Referencia pública | Equivalencia |
| --- | --- | --- | --- |
| `granulometry` | % retenido acumulado, % que pasa, **D10/D30/D60**, **Cu** y **Cc** | TxDOT Tex-110-E §5 | D6913 / T 27 |
| `atterberg` | Humedad, **límite líquido** (un punto y multipunto), **límite plástico**, **IP** | Tex-104-E §8, Tex-105-E §5 | D4318 / T 89 y T 90 |
| `compaction` | Densidad húmeda, humedad, **densidad seca**, y **máximo de la curva** (OMC y MDD) | Tex-113-E §8.5–8.7, FM 1-T 180 | D698 / D1557 |
| `cbr` | **CBR** a partir de las cargas corregidas y de la curva completa | ODOT *Checklist T-193*; FDOT FM 5-515 §7.1 | D1883 / T 193 |
| `field_density` | **Densidad seca en sitio**, grado de compactación y desviación de humedad | Tex-115-E §6.1 | D6938 / D1556 |

## CBR: la carga estándar, verificada

El checklist público de Ohio DOT (T-193/D1883, mayo 2024) confirma el divisor: **6,9 y
10,3 MPa (1000 y 1500 psi)** a 2,54 y 5,08 mm, multiplicando por 100. El método de Florida
que se había encontrado divide entre **800 psi** porque es LBR, no CBR. Codificar ese
divisor habría dado valores altos en todos los suelos.

## Clasificación SUCS y AASHTO: **bloqueada, con evidencia**

Se intentó con tres fuentes públicas:

| Fuente | Problema |
| --- | --- |
| FHWA NHI-06-088, tabla 4-13 | La tabla está en una página rotada; la extracción de texto mezcla columnas. La imagen se pudo leer, pero la fila de A-2 deja una celda combinada cuyo alcance (¿No. 10, No. 200 o ambas?) no se puede determinar con certeza |
| Manual WAQTC/CDOT de inspección de suelos | La tabla aparece aplanada y **contradice** a FHWA en qué fila va el 51 min de A-3 |
| Wikipedia y apuntes de curso | Reproducen la tabla, pero no son fuente normativa ni coinciden entre sí en los mismos detalles |

Con dos fuentes públicas que se contradicen en valores concretos, escribir la clasificación
sería inventar. **No se implementa** hasta tener D2487 y D3282 (o el pack verificado del
cliente). El producto no clasifica suelos hoy, y lo dice: no hay ninguna rama que devuelva
un símbolo SUCS aproximado.

Detalle que sí quedó resuelto y sirve para el día que se implemente: la fórmula del **índice
de grupo** está transcrita del manual de FHWA (ecuación 4-3) y sus reglas de redondeo
(GI negativo → 0; redondeo al entero; sin tope superior; GI = 0 para A-1-a, A-1-b, A-2-4,
A-2-5 y A-3; para A-2-6 y A-2-7 solo el término de plasticidad).

## Decisiones que importan

- **La interpolación de D10/D30/D60 es logarítmica**, porque la granulometría se lee así.
  Interpolar linealmente entre 2 mm y 0,075 mm daría un diámetro equivocado.
- **El máximo de la curva se ajusta con una parábola a los tres puntos que rodean al
  mayor.** Si el mayor está en un extremo, la curva seguiría subiendo o bajando y no hay
  máximo: se devuelve `None` en lugar de un número inventado. Un OMC obtenido de dos puntos
  no es un resultado, es una recta.
- **Datos incoherentes devuelven error, no un número raro**: masa retenida mayor que la
  muestra, límite plástico mayor que el líquido, tara mayor que la masa seca, cero golpes.
- **Los grados de libertad del método se respetan**: el multipunto exige al menos dos
  determinaciones con golpes distintos; el de un punto aplica `LL = W_N·(N/25)^0,121`.

## Evidencia

```
cargo test -p mamut-calculations   → 44 pruebas, 0 fallos
```

Las 25 pruebas nuevas cubren: el complemento retenido/pasa, el orden de los tamices, D10/D30/D60
con una curva de resultado conocido (Cu = 8, Cc = 0,889), la interpolación semilogarítmica,
el método de un punto en 25 y fuera de 25 golpes, el plástico por masas, la monotonía de la
densidad seca con la humedad y el vértice de la curva.

## Lo que falta en la fase 3

| Pieza | Estado |
| --- | --- |
| Clasificación **SUCS** (D2487) y **AASHTO** (D3282) | **Bloqueada**: dos fuentes públicas se contradicen en valores concretos. Requiere la norma |
| **CBR** | Cálculo, pantalla e informe hechos |
| **Densidad de campo** | Cálculo, pantalla e informe hechos (gauge nuclear; el cono de arena quedó fuera de Tex-115) |
| **Granulometría** | Cálculo, pantalla e informe PDF hechos |
| **Límites de Atterberg** | Cálculo, pantalla e informe hechos |
| **Compactación** | Cálculo, pantalla e informe hechos |
| Interfaz de los ensayos de suelos | Hecha: un panel con los cinco ensayos |
| Sección de suelos en el informe PDF | Hecha: un informe por ensayo |
| Gráfico granulométrico semilogarítmico | La geometría existe (`Scale::Log10`); falta montarlo |

## Siguiente

1. Interfaz del ensayo de granulometría (la más usada) y su sección de informe.
2. Límites de Atterberg y Proctor en la interfaz.
3. Decidir la fuente para SUCS/AASHTO y para CBR antes de prometerlos.
