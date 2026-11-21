# Fase 0 · Investigación de Rune

> **Estado:** cerrado para decisión. Fecha del análisis: 2026-11-18.
> **Revisión inspeccionada:** `4a2e437954f01eaacc13c0515a1d39474dd6512c` (rama `dev`),
> commit del **2026-11-18 13:22 (-05:00)**, es decir, del mismo día de este análisis.
>
> Todo lo que sigue se verificó contra el repositorio clonado y su código, no contra
> resúmenes de terceros. Cuando un dato proviene de una fuente externa, se indica.

---

## 1. Método

1. Se clonó `https://github.com/anomalyco/rune` (rama `dev`, ~99 MB, 3.446 archivos).
2. Se leyó la documentación del propio repositorio: `README.md`, `AGENTS.md`,
   `crates/rune/README.md` y las notas de `research/`.
3. Se inspeccionó el código fuente de las APIs que MAMUT Lab necesita, para no
   depender de documentación desactualizada.
4. Se leyó la batería de pruebas del propio framework (`crates/rune/tests/`, 130+
   archivos) para entender cómo se valida a sí mismo.
5. Se construyó una aplicación de prueba real (ver `02-validacion-stack.md`).

Cada afirmación de capacidad en este documento cita el archivo donde se verificó.

---

## 2. Qué es Rune

Rune es un framework de interfaz **nativo para Rust**, renderizado por GPU, de la
empresa Anomaly. No es un envoltorio de tecnologías web: implementa su propio
renderizador, su propio motor de texto y su propio sistema de animación.

Según `crates/rune/Cargo.toml` y `crates/rune/README.md`, la pila interna es:

| Componente | Tecnología | Versión fijada |
| --- | --- | --- |
| GPU / ventanas | `wgpu` | `~30.0.1` |
| Ventanas nativas | `winit` | `0.30.13` (vendorizado) |
| Layout | `taffy` | `0.15` |
| Texto | `parley` + `swash` + `skrifa` + `fontique` | `0.12` / `0.2.10` / `0.44` |
| Accesibilidad | `accesskit` | `0.25.2` |
| Animación | `rune-motion` (muelles analíticos) | propio |

El renderizador propio dibuja en **una sola llamada de dibujo por frame**
(`crates/rune/README.md`, sección *Renderer*): rectángulos redondeados, sombras
gaussianas analíticas, degradados lineales/radiales, imágenes RGBA, glifos desde
atlas y trazos vectoriales. Eso es relevante para MAMUT Lab porque significa que
**los gráficos técnicos se pueden dibujar de forma exacta y controlada**, sin
depender de una librería de gráficos de terceros.

### Plataformas

`AGENTS.md:6` declara macOS, Windows, Linux, web e iOS. En el repositorio hay
evidencia de las tres primeras y de web (ejemplos `web-hello`, `web-showcase`,
`www/`). Para MAMUT Lab la plataforma objetivo inicial es **Windows**, que es
además la que se validó en esta fase.

---

## 3. Estado, madurez y gobernanza

Esto es lo que más condiciona la decisión, así que se expone sin adornos.

| Hecho | Evidencia | Implicación |
| --- | --- | --- |
| Versión `0.0.1`, **pre-release** | `crates/rune/Cargo.toml:3` | La API puede cambiar entre revisiones. |
| Distribución por **dependencia git a una revisión de `dev`** | `README.md`, *Add it* | Obligatorio fijar revisión; no hay versión en crates.io. |
| `dev` es **fast-forward only** y nunca se reescribe | `AGENTS.md` | Clave: fijar una revisión es seguro y reproducible. |
| **No aceptan pull requests** | `README.md:63` | No se puede corregir el framework por la vía normal: solo reportar issues. |
| Último commit: **2026-11-18 13:22**, el mismo día del análisis | `git log -1` local | Desarrollo diario y muy activo. Actividad = señal buena de salud, señal mala de estabilidad. |
| CI de macOS corre **solo por agenda**, no por PR | `AGENTS.md:20` | Riesgo de regresiones en macOS entre revisiones. Windows y Linux no tienen esa restricción documentada. |
| Licencia **MIT** | `LICENSE` | Compatible con producto comercial propietario. |
| Equipo pequeño y de una sola empresa | `AGENTS.md`, autoría de commits | Riesgo de dependencia de proveedor (*bus factor*). |
| Las apps deben replicar el bloque `[patch.crates-io]` de Rune | `crates/rune/README.md`, *Depending on Rune* | Coste de integración real, detallado en §5. |

> Datos externos, no verificados en el repositorio: ~6.9k estrellas en GitHub y
> ~6 contribuidores (análisis de terceros, noviembre 2026). Se citan solo como
> contexto de comunidad, no como base de decisión.

### Valoración honesta

Rune es **técnicamente serio** —no es una maqueta—: tiene renderizador propio,
motor de texto con comparaciones medidas contra CoreText y DirectWrite, pruebas
de accesibilidad por plataforma y una batería de más de 130 pruebas de integración.
Pero es **pre-release, de un solo proveedor y sin vía de contribución externa**.
Un producto comercial no puede ignorar eso; la respuesta correcta no es descartarlo
ni confiar ciegamente, sino **acotar el riesgo con arquitectura** (ADR-0008).

---

## 4. Capacidades verificadas que MAMUT Lab necesita

| Necesidad del producto | ¿Existe en Rune? | Evidencia |
| --- | --- | --- |
| Ventana nativa, tema claro/oscuro | Sí | `rune::tok` (tokens OKLCH con temas), `examples/hello` |
| Navegación lateral y shell de escritorio | Sí, componentes | `rune-kit`: `Sidebar`, `SplitView`, `TabStrip`, `Toolbar`, `WindowTitlebar` (`crates/rune-kit/src/lib.rs`) |
| **Tabla técnica con muchas filas** | Sí, **virtualizada** | `rune-kit/src/table.rs`: cabecera fija, columnas ordenables y redimensionables, selección y navegación por teclado; «solo se construyen las filas visibles… 100.000 filas cuestan lo que las treinta visibles» |
| Formularios y campos de texto | Sí | `TextField`, `TextInput`, `TextArea`; IME, deshacer, selección, campos seguros (`crates/rune/README.md`, *Text fields*) |
| Diálogos de archivo nativos | Sí, tras feature | `rune` con `native-file-dialog`: `rfd 0.17` en Windows/macOS, portal XDG en Linux; API `tx.open_file(options, cb)` (`README.md`, *File dialogs*) |
| Menús nativos y atajos | Sí | `native_menu`, `shortcuts`, `Chord::parse("mod+shift+t")` |
| Notificaciones locales | Sí | `rune-notify`, `rune_kit::Toast`/`ToastStack` |
| Carpetas de datos por usuario | Sí | `rune::dirs::AppDirs::new("com.empresa.app")` → `config/data/cache/state/logs`, con `RUNE_DIRS_ROOT` para aislar pruebas |
| Internacionalización (es/en) | Sí | crate `rune-i18n` (catálogos JSON, plurales CLDR) |
| Accesibilidad (UI Automation en Windows) | Sí, tras feature | `accessibility` + parches `accesskit_windows`/`accesskit_macos` |
| Portapapeles enriquecido (pegar imágenes) | Sí | `ClipboardContent`, `content.image()` |
| **Gráficos técnicos** | **No hay componente** | Se dibujan con primitivas vectoriales: `Shape::stroke`, `Shape::fill`, `Path`, `El::shape`, `canvas()` (`crates/rune/src/shape.rs`) |
| **Generación de PDF** | **No** | Fuera del alcance del framework; se resuelve en `lab-reports` |
| **Pruebas de interfaz sin ventana** | **Sí, y es excepcional** | Ver §6 |

---

## 5. Modelo de integración (el coste real)

Rune **no se hereda como una dependencia normal**. Su `Cargo.toml` raíz parchea
tres crates de crates.io, y Cargo no propaga parches desde una dependencia. La
documentación oficial (`crates/rune/README.md`, *Depending on Rune*) indica que la
aplicación debe replicar el bloque completo en su propio workspace:

```toml
[dependencies]
rune = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }

[patch.crates-io]
winit          = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }
parley_engine  = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }
wgpu-hal       = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }
accesskit_windows = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }
accesskit_macos   = { git = "https://github.com/anomalyco/rune", rev = "<rev>" }
```

Consecuencias que un equipo debe asumir:

1. **Cada actualización de Rune exige actualizar también las revisiones de los parches.**
   Es un procedimiento, no un `cargo update`. Debe quedar escrito y automatizado.
2. **Sin los parches, Rune sigue compilando.** Se verificó en
   `crates/rune/src/gpu/pick.rs:159-169`: si `wgpu-hal` no está parcheado, existe un
   *trait* de respaldo que devuelve `None` y la selección de adaptador cae al método
   estándar de `wgpu`. El coste es de rendimiento (en portátiles con dos GPU, medir
   cada adaptador puede costar ~1 s de arranque), **no de compilación**.
   Esto baja mucho el riesgo de la fase 1: se puede empezar sin parches y añadirlos
   después como optimización medida.
3. **Los parches son código de terceros fijado por revisión**, no una versión publicada.
   Para un producto comercial conviene registrar en el repositorio propio la revisión
   exacta y la razón de cada parche.
4. **Linux** exige `libxkbcommon` y `libwayland` en tiempo de compilación
   (`README.md`, *Linux*). Afecta a CI, no a Windows.

---

## 6. La capacidad decisiva: pruebas de interfaz sin ventana

Rune puede ejecutar una aplicación **sin ventana, sin GPU visible y con reloj virtual**,
y exponer su árbol de elementos. Verificado en:

- `crates/rune/tests/headless_place.rs` — ejecuta una app con `--script` y `--tree`,
  parsea el JSON del árbol y hace aserciones sobre posiciones.
- `examples/hello/src/main.rs` (bloque `mod tests`) — patrón de prueba unitaria:

```rust
let env = Env { system_dark: true, scale: 2.0 };
let mut rt = Runtime::new(State::default(), view, (320.0, 200.0), env);
rt.frame(0.0);
let (x, y) = rt.find("switch").unwrap();      // centro del elemento por su clave
rt.pointer_down(x, y);
rt.pointer_up(x, y);
for _ in 0..6 { rt.frame(1.0 / 60.0); }       // reloj virtual: determinista
assert!(rt.state.on);
```

Firmas verificadas en el código: `Runtime::new` (`runtime.rs:901`),
`Runtime::find(&self, key) -> Option<(f32, f32)>` (`el/elements.rs:389`),
`Runtime::presented(&self, key) -> Option<([f32; 4], f32)>` (`el/elements.rs:406`),
`Runtime::frame(&mut self, dt: f64) -> bool` (`el/frame.rs:378`),
`Runtime::pointer_down/up` (`el/pointer.rs:206`, `:342`).

Además existen los modos de línea de comandos, verificados en `args.rs`:
`--snap <png>`, `--tree <json>`, `--script <json>`, `--size`, `--theme`, `--scale`,
`--fps`, `--at`.

**Por qué esto importa para un laboratorio:** permite probar de verdad que un
formulario rechaza una masa en cero, que el botón «Calcular» produce el estado
esperado y que la tabla muestra el ensayo recién creado — sin abrir una ventana y
sin un humano mirando. Es exactamente el tipo de prueba que un software técnico
necesita para no fallar en silencio. Y los diálogos de archivo tienen *fixtures*
guionizados (`FileDialogs::scripted`), así que también se pueden probar las
importaciones.

---

## 7. Lo que Rune no resuelve (y hay que construir)

1. **Gráficos.** No hay componente de gráficos. Rune ofrece trazos y rellenos
   vectoriales; la escala, los ejes, las marcas, las bandas de especificación y la
   exportación serán código propio. Se considera **una ventaja**: un gráfico de
   laboratorio debe mostrar límites normativos, incertidumbre y ejes con unidades,
   y las librerías genéricas se resisten a eso.
2. **Informes PDF.** Se generan con una crate aparte (ADR-0005).
3. **Instalador, firma de código y actualizaciones.** Rune no los incluye; son
   trabajo de la fase 6.
4. **Iconos de aplicación por plataforma, bandeja del sistema**, etc.: existen
   (`tray`, `rune-mac`), pero su empaquetado es responsabilidad del producto.

---

## 8. Comparación con alternativas

No se evaluó cada alternativa con la misma profundidad; se contrastan por lo que
afecta a este producto.

| Opción | A favor | En contra para MAMUT Lab |
| --- | --- | --- |
| **Rune** (elegida) | Nativo, rápido, tabla virtualizada incluida, pruebas headless excepcionales, MIT, desarrollo diario | Pre-release, un proveedor, sin PRs, obligación de fijar revisión |
| `egui` | Maduro, enorme adopción, inmediato | Estética de herramienta interna; tablas y formularios de calidad comercial requieren mucho trabajo; su modelo inmediato dificulta separar vista y estado |
| `iced` | Elm-like, maduro, buen soporte | Sin `accesskit` integrado en la rama principal (según `research/02-rust-ui-ecosystem.md`), componentes de tabla/formulario a medida |
| `Slint` | Excelente calidad visual, DSL declarativa | DSL propia + licencia dual GPLv3/comercial: coste y ataduras para producto propietario |
| `GPUI` (Zed) | Muy rápido, base sólida | API de bajo nivel (hay que construir los 75+ controles), acoplado al ecosistema de Zed |
| Tauri / Electron | Ecosistema web, informes HTML fáciles | WebView: consumo de memoria, arranque, dependencia del motor del sistema; el requisito pide nativo sin justificación explícita |

Rune es la única opción que combina **nativo real, componentes de calidad
profesional ya existentes y un arnés de pruebas determinista**. Esa combinación es
la que justifica la elección, no la novedad.

**No se introduce Electron, Tauri ni React.** El requisito de un producto nativo,
offline-first y de larga vida no lo justifica, y añadiría una superficie de
seguridad y empaquetado mayor.

---

## 9. Veredicto

**Se adopta Rune**, con cuatro condiciones no negociables:

1. **Revisión fijada** en todo el workspace, y una única fuente de verdad para ella.
2. **Aislamiento de la interfaz**: ningún cálculo, norma ni regla de negocio puede
   importar `rune`. El dominio debe compilar y probarse sin el framework.
3. **Plan de salida**: si Rune deja de ser viable, se sustituye la capa de
   presentación sin reescribir el producto. Ver ADR-0008.
4. **Registro de deuda**: toda limitación del framework que obligue a una solución
   temporal se anota en `docs/deuda-tecnica.md`.

El detalle de por qué se acepta este riesgo está en **ADR-0001** y **ADR-0008**.
