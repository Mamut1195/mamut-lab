# Fase 0 · Validación del stack (evidencia ejecutada)

> Todo lo de este documento se ejecutó en la máquina de desarrollo el **2026-11-18**.
> No son estimaciones ni citas: son resultados medidos.

## 1. Entorno

| Elemento | Valor |
| --- | --- |
| SO | Windows (Windows 11, x64) |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, `cargo 1.95.0` |
| Target | `x86_64-pc-windows-msvc` |
| GPU | AMD Radeon(TM) Graphics (integrada), backend **DirectX 12** vía `wgpu` |
| Rune | revisión `4a2e437954f01eaacc13c0515a1d39474dd6512c` (rama `dev`) |
| Prueba | `spikes/rune-probe` (aplicación real, no un «hola mundo») |

## 2. Qué se construyó

Una aplicación Rune con la forma del producto: barra lateral de navegación, tira de
indicadores, **tabla técnica virtualizada de 240 filas** con columnas ordenables y
redimensionables, **gráfico de líneas** dibujado con primitivas vectoriales, barra de
estado y conmutador de tema claro/oscuro. En el mismo binario: SQLite embebido,
exportación XLSX y la crate de PDF.

Dependencias resueltas: **528 paquetes**; compilados: **299 crates**.

## 3. Resultados

### Compilación

```
cargo build --bin rune-probe   →  Finished `dev` profile in 3m 22s   (exit 0)
```

Compilaron correctamente, entre otros: `rune`, `rune-kit`, `rune-motion`,
`rune-staging`, `rune-taffy`, `taffy 0.15.1`, `parley 0.12`, `wgpu 30.0.2`,
`wgpu-hal 30.0.1` **(parcheado)**, `winit 0.30.13` **(parcheado)**,
`parley_engine 0.12.0` **(parcheado)**, `accesskit_windows 0.35.2` **(parcheado)**,
`rusqlite 0.43.0` (SQLite embebido), `rust_xlsxwriter 0.102.0`, `printpdf 0.12.11`.

**Conclusión: el stack completo compila y enlaza en un solo proceso nativo en Windows.**

### Pruebas automáticas

```
cargo test --bin rune-probe

running 5 tests
test tests::chart_geometry_maps_values_into_the_box ... ok
test tests::sqlite_round_trip ... ok
test tests::shell_lays_out ... ok
test tests::nav_press_changes_state ... ok
test tests::xlsx_export_writes_a_file ... ok

test result: ok. 5 passed; 0 failed; finished in 0.38s
```

Qué demuestra cada una, y por qué importa:

| Prueba | Qué valida realmente |
| --- | --- |
| `shell_lays_out` | El runtime headless de Rune construye la vista, ejecuta el layout y expone rectángulos de los elementos por su clave (`sidebar`, `table-frame`, `chart`) |
| `nav_press_changes_state` | Entrada simulada real (`pointer_down`/`pointer_up` sobre el centro de un elemento) cambia el estado de la aplicación de forma determinista |
| `chart_geometry_maps_values_into_the_box` | La geometría del gráfico es pura y comprobable sin GPU, incluidos casos degenerados (1 punto, serie constante) |
| `sqlite_round_trip` | SQLite embebido crea esquema, inserta y consulta, con distinción correcta entre «no existe» y «existe» |
| `xlsx_export_writes_a_file` | XLSX escribe un libro real (contenedor ZIP > 1 KB) en disco |

**Conclusión: la interfaz es probable en `cargo test`, con reloj virtual, sin abrir
ventana y en menos de medio segundo.** Esto es lo que hace viable la disciplina de
pruebas del ADR-0007.

### Render real por GPU

```
rune-probe --snap probe-dark.png  --size 1180x800 --theme dark
rune-probe --snap probe-light.png --size 1180x800 --theme light
```

```
rune: GPU "AMD Radeon(TM) Graphics" (wgpu; Dx12, IntegratedGpu): lowest power
rune: 1 frames at 60 fps, gpu init 112.7 ms, first frame 281.63 ms,
      frame cpu mean 281.626 ms, 770 instances, layout nodes 397, glyphs 675
```

Salida: `probe-dark.png` y `probe-light.png`, **2360×1600 px** (escala 2×), ~244 KB.
Ambas imágenes se revisaron visualmente: jerarquía correcta, tabla alineada a la
derecha en columnas numéricas, cabecera fija, filas alternas, gráfico vectorial
nítido y barra de estado con el aviso de datos de prueba.

**Conclusión: el renderizado por GPU funciona en hardware real (DX12), y el mismo
binario produce las dos variantes de tema.**

### Rendimiento observado (contexto, no un benchmark)

| Métrica | Valor | Lectura |
| --- | --- | --- |
| Inicio de GPU, primera ejecución | 11.809 ms | Compilación en frío de shaders; solo la primera vez |
| Inicio de GPU, ejecución siguiente | 112,7 ms | Caché de shaders en disco |
| Primer frame (build **debug**) | 281–364 ms | Build sin optimizar; no representa el producto final |
| Nodos de layout / glifos | 397 / 675 | Escena pequeña y realista |

No se ha medido un build `--release`; debe hacerse en la fase 1 como línea base. Los
11,8 s de la primera ejecución son una advertencia útil para el arranque del producto:
hay que medir y, si molesta, considerar precompilar shaders.

## 4. Hallazgos que cambian el plan

### H-1 · El bloque `[patch.crates-io]` es **obligatorio** (bloqueante)

Sin los parches, la resolución falla:

```
package `rune` depends on `parley_engine` with feature `line-dictionaries`
but `parley_engine` does not have that feature
```

`rune` requiere una feature que **solo existe en la copia vendorizada** de
`parley_engine` dentro del repositorio de Rune. No es una optimización: es un
requisito de compilación. El bloque de la documentación de Rune se copió completo
(cinco parches, misma revisión) y la resolución pasó a 528 paquetes sin errores.

**Consecuencia para el producto:** la revisión de Rune y las cinco de los parches se
actualizan juntas, siempre. Debe quedar automatizado y documentado (ADR-0008 §5).

### H-2 · `cargo` se colgaba descargando la dependencia git (bloqueante en Windows)

Con el cliente git interno de Cargo (`libgit2`), la descarga se quedaba en 0 MB
indefinidamente. Se resolvió con:

```powershell
$env:CARGO_NET_GIT_FETCH_WITH_CLI = "true"
```

**Consecuencia para el producto:** debe fijarse en el repositorio
(`.cargo/config.toml`) y en CI. Sin esto, un desarrollador nuevo no consigue compilar
y el problema parece un fallo de red.

### H-3 · Los parches de accesibilidad resuelven correctamente

`accesskit_windows` y `accesskit_macos` se parchearon por revisión y compilaron. La
accesibilidad real (UI Automation) **no se probó** porque la feature no estaba
activada en la prueba; queda como verificación de la fase 1.

### H-4 · El árbol de dependencias es grande

528 paquetes resueltos. No es un problema de producto (todo se compila dentro del
binario), pero sí de tiempos de compilación en CI: conviene caché de `target` y
`sccache` desde la fase 1.

## 5. Lo que **no** se ha validado (honestidad de alcance)

| Pendiente | Por qué | Cuándo |
| --- | --- | --- |
| Compilación en Linux y macOS | Solo hay esta máquina Windows | Fase 1 (CI con matriz) |
| Accesibilidad real (lector de pantalla) | Feature desactivada en la prueba | Fase 1 |
| Apertura real de un diálogo de archivo | Compilado, no ejercitado (requiere interacción) | Fase 2, con `FileDialogs::scripted` |
| API de `printpdf` para maquetar tablas | Solo se validó que compila | Inicio de fase 2 (prueba de concepto) |
| Build `--release` y perfil de rendimiento | Fuera del alcance de la fase 0 | Fase 1 |
| Instalador, firma de código, actualizaciones | No existen en Rune | Fase 6 |
| Texto normativo de ASTM/AASHTO/INEN | Requiere adquisición con licencia | Antes de fase 2 |

Ninguna de estas ausencias invalida la decisión: son verificaciones posteriores con
riesgo bajo o ya mitigado.

## 6. Cómo reproducir

```powershell
cd spikes/rune-probe
$env:CARGO_NET_GIT_FETCH_WITH_CLI = "true"
cargo test --bin rune-probe
cargo run -- --snap probe.png --size 1180x800 --theme dark
```

La aplicación de prueba es **desechable**: existe para producir esta evidencia. Su
código no forma parte del producto y se eliminará (o se archivará) al crear el
workspace real en la fase 1.
