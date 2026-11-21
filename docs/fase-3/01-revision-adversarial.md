# Fase 3 · Revisión adversarial

**Fecha:** 2026-11-21 · **Revisor:** agente independiente, sin acceso a mis razonamientos
**Veredicto inicial:** RECHAZADO (5 graves, 11 menores)

La revisión se hizo sobre los módulos de cálculo con el dosier de referencias públicas como
patrón. Lo importante no es el veredicto: son los cinco fallos que encontró y que **ninguna
de las 59 pruebas existentes detectaba**.

## Hallazgos graves y qué se hizo

### G1 · Dos convenciones de humedad incompatibles

`moisture_percent(wet, dry)` calculaba `100·(húmedo−seco)/seco`, sin tara, mientras
`plastic_limit(agua, seco, tara)` sí la restaba. Las mismas pesadas (A=220, B=200, C=50)
daban **10,00 %** donde la norma da **13,33 %**.

- **Corregido:** una sola función con las masas del recipiente.
- **Prueba añadida:** 220/200/50 → 13,33 %.

### G2 · El máximo de compactación podía inventarse

`curve_peak` ajustaba una parábola a los tres puntos que rodean al mayor. Con
(8, 1,85) (10, 1,95) (20, 1,80) devolvía **2,021 g/cm³**, un 3,6 % por encima de todo lo
medido. Ese número alimenta el grado de compactación: un 98,4 % real se informaba como
**95,0 %** — un rechazo de obra falso.

- **Corregido:** mínimos cuadrados sobre todos los puntos, más cotas: el vértice debe caer
  dentro del rango ensayado y no puede superar en más de un 1 % el máximo medido.
- **Pruebas añadidas:** el caso (8/10/20) devuelve `None`; un caso de cinco puntos da
  12,16 % y 1,9473 g/cm³ con `points_used = 5`.

### G3 · La tolerancia de pesada rechazaba ensayos válidos

`sum > total·(1+1e-9)` son 5·10⁻⁷ g para 500 g. Cualquier juego redondeado a 0,1 g se
rechazaba, con un mensaje que se contradecía a sí mismo al imprimir ambos valores.

- **Corregido:** tolerancia de 0,2 % del total con mínimo 0,1 g.
- **Prueba añadida:** 500,05 g sobre 500,0 g se acepta.

### G4 · El límite líquido aceptaba datos que el método no admite

Aceptaba 7 golpes, dos determinaciones (sin grados de libertad) y pendiente positiva. El
docstring prometía una «capa de validación» que no existía en ningún crate.

- **Corregido:** exige ≥3 determinaciones, rechaza pendiente ≥ 0 y devuelve pendiente, R² y
  aviso de golpes fuera de rango.
- **Prueba añadida:** pendiente positiva → error.

### G5 · Un CBR sin saber qué penetración gobernó ni si se corrigió

Devolvía solo un número. `cbr_percent` era pública y aceptaba cargas sin corregir.

- **Corregido:** devuelve `percent`, `governing_penetration_mm` y `offset_mm`; rechaza
  cargas ≤ 0 y CBR > 200 con aviso sobre unidades.

## Menores corregidos

| # | Qué era | Corrección |
| --- | --- | --- |
| M1 | Divisor 1500 psi redondeado a 10,3 (sesgo +0,41 %) | Constantes derivadas de psi: 6,894757 y 10,342136 |
| M2 | Redondeo desbordaba a infinito | Devuelve el valor si la escala no es finita |
| M3 | Índice de plasticidad aceptaba límites negativos | Exige ambos positivos |
| M4 | Tamiz repetido con tolerancia absoluta | Igualdad exacta |
| M5 | Pruebas de interpolación tautológicas | Caso asimétrico 70 %→10 %: D60 1,545 · D30 0,712 |
| M6 | Prueba del divisor del CBR tautológica | Anclada a 3,4474 y 5,1711 MPa |
| M7 | Corrección de origen sin cota | ≤1 mm y ≤20 % de 5,08; devuelve `Option` |
| M8 | Densidad en sitio duplicaba la fórmula y no avisaba | Delega en `compaction::dry_density` y devuelve avisos de plausibilidad |
| M10 | D en el borde de la curva sin marcar | `sieve_count` y `d_at_boundary` en el resultado |

## Lo que esta revisión demuestra

- **Las pruebas propias no bastan.** 59 pruebas en verde y cinco fallos graves: los tests
  habían sido escritos por quien escribió el código, y varios fijaban la convención
  equivocada (el de la humedad) o eran tautológicos (el del divisor del CBR).
- **El fallo más caro no era aritmético, era de contrato:** un número que no dice qué
  significa ni con qué se corrigió acaba en un informe sin que nadie pueda auditarlo.

## Estado tras las correcciones

```
cargo clippy -D warnings → 0
cargo test --workspace   → 375 pruebas, 0 fallos
```

Pendiente de decisión, no de trabajo: la meseta en la interpolación (se devuelve el diámetro
más fino, documentado) y el docstring de `lib.rs`.
