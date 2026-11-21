# MAMUT Lab

Software de escritorio para laboratorios de ensayo de materiales de construcción:
muestras, ensayos, cálculos reproducibles, trazabilidad e informes técnicos.

**Estado:** Fase 1 cerrada; **Fase 2 en curso**. Ya funciona el ciclo completo
**muestra → ensayo de compresión → informe PDF**: alta de muestras con validación y
auditoría, ensayo con cálculo reproducible, historial por muestra e informe técnico en PDF
generado sin dependencias. Faltan los adjuntos, el gráfico y la acción de exportación.

---

## Documentación

| Documento | Para qué |
| --- | --- |
| [Resumen ejecutivo](docs/fase-0/00-resumen-ejecutivo.md) | **Empieza aquí.** Decisiones que requieren aprobación |
| [Investigación de Rune](docs/fase-0/01-investigacion-rune.md) | Estado real del framework, capacidades verificadas |
| [Validación del stack](docs/fase-0/02-validacion-stack.md) | Evidencia ejecutada y límites de la validación |
| [Arquitectura](docs/fase-0/03-arquitectura.md) | Modelo de datos, flujo de un ensayo, estado de la app |
| [Roadmap](docs/fase-0/04-roadmap.md) | Fases, criterios de salida, esfuerzo |
| [Riesgos](docs/fase-0/05-riesgos.md) | Registro con mitigaciones |
| [Normas sin bloqueo](docs/fase-0/06-normas-sin-bloqueo.md) | Cómo se avanza sin las normas y cómo entran al final |
| [Norm pack](docs/normas/README.md) | Formato de datos y flujo de revisión normativa |
| [ADRs](docs/adr/) | Las nueve decisiones de arquitectura |
| [Deuda técnica](docs/deuda-tecnica.md) | Lo que se asume conscientemente |

## Stack

Rust · **Rune** (pre-release, revisión fijada) · SQLite (`rusqlite`, embebido) · Serde ·
`rust_xlsxwriter`. `printpdf` entra en la fase 2 (hoy no es dependencia del producto).

Detalle y justificación en [ADR-0001](docs/adr/0001-framework-ui-rune.md),
[ADR-0002](docs/adr/0002-persistencia-sqlite.md) y
[ADR-0005](docs/adr/0005-informes-pdf.md).

---

## Requisitos de desarrollo

- Rust **1.95** o superior (`rustup default stable`).
- Windows con las Build Tools de Visual Studio (target `x86_64-pc-windows-msvc`).
- Conexión a Internet **solo** para la primera descarga de dependencias. La aplicación
  funciona sin conexión.

### Nota imprescindible en Windows

`cargo` se queda bloqueado descargando la dependencia git de Rune si usa su cliente
interno. El repositorio incluye la corrección en `.cargo/config.toml`:

```toml
[net]
git-fetch-with-cli = true
```

Si compilas fuera de este repositorio, define `CARGO_NET_GIT_FETCH_WITH_CLI=true`.

## Prueba de validación (Fase 0)

`spikes/rune-probe` es una aplicación desechable que demuestra que el stack compila,
se prueba y renderiza en esta máquina. **No forma parte del producto.**

```powershell
cd spikes/rune-probe
cargo test --bin rune-probe          # 5 pruebas: UI headless, SQLite, XLSX, geometría
cargo run -- --snap probe.png --size 1180x800 --theme dark
```

Resultados medidos y su interpretación en
[02-validacion-stack.md](docs/fase-0/02-validacion-stack.md).

---

## Principios del producto

1. **Exactitud numérica antes que estética.** Un informe mal calculado es un daño,
   no un bug.
2. **Nada de fórmulas inventadas.** Cada cálculo cita su norma y su versión; sin norma
   revisada no se emite juicio de conformidad
   ([ADR-0006](docs/adr/0006-normas-conformidad-trazabilidad.md)).
3. **Los datos originales no se tocan.** Corregir crea una versión nueva, con motivo
   y autor.
4. **Offline-first de verdad.** SQLite y adjuntos locales; ninguna dependencia de
   servidores.
5. **La interfaz es reemplazable.** El dominio se compila y se prueba sin el framework
   ([ADR-0008](docs/adr/0008-aislamiento-y-plan-de-salida.md)).

## Fuera de alcance

ERP de construcción, cálculo estructural, BIM, presupuestos, cubicaciones, portal web
para clientes. El dominio es el laboratorio de materiales y la trazabilidad de ensayos.

## Licencia

**MIT.** El código puede usarse, modificarse, distribuirse y venderse, conservando el aviso
de copyright. Ver [LICENSE](LICENSE).

Las dependencias de terceros son MIT/Apache-2.0 en su práctica totalidad; Rune es MIT. Antes
de distribuir un binario conviene ejecutar `cargo audit` y revisar las licencias del árbol
completo (deuda DT-18).

> **Nota:** el titular del copyright en `LICENSE` es «MAMUT Lab». Si prefieres tu nombre
> legal o el de una empresa, es la única línea que hay que cambiar.
