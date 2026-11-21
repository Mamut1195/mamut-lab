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
| Clasificación **SUCS** (D2487) y **AASHTO** (D3282) | Pendiente: las tablas de clasificación son el corazón de la norma y no aparecen en los procedimientos públicos. Requiere la norma o una fuente equivalente verificada |
| **CBR** | Pendiente: lo encontrado es LBR (800 psi), que **no** es CBR (1000/1500 psi). No se implementa mal a sabiendas |
| Interfaz de los ensayos de suelos | Pendiente: el cálculo existe; falta el espacio de trabajo como el de compresión |
| Sección de suelos en el informe PDF | Pendiente |
| Gráfico granulométrico semilogarítmico | La geometría existe (`Scale::Log10`); falta montarlo |

## Siguiente

1. Interfaz del ensayo de granulometría (la más usada) y su sección de informe.
2. Límites de Atterberg y Proctor en la interfaz.
3. Decidir la fuente para SUCS/AASHTO y para CBR antes de prometerlos.
