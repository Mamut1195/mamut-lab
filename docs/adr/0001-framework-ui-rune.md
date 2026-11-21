# ADR-0001 · Framework de interfaz: Rune

- **Estado:** aceptado (condicionado a ADR-0008)
- **Fecha:** 2026-11-18
- **Decisores:** arquitectura, UX, QA

## Contexto

MAMUT Lab debe ser una aplicación **nativa de escritorio**, offline-first, con
apariencia de software técnico comercial, tablas densas, formularios rigurosos y
gráficos. El stack preferido fija **Rust** y **Rune** (Anomaly).

Rune es pre-release (`0.0.1`), se consume como dependencia git a una revisión de
`dev`, no admite pull requests y exige replicar su bloque `[patch.crates-io]` en la
aplicación. Ver `01-investigacion-rune.md` para la evidencia completa.

## Decisión

Se adopta **Rune** en la revisión `4a2e437954f01eaacc13c0515a1d39474dd6512c`,
fijada en **un único lugar** del workspace, con estas condiciones:

1. **Ningún crate de dominio puede depender de `rune`.** Los cálculos, las normas,
   la validación y la persistencia se compilan y se prueban sin el framework.
2. La actualización de Rune es un **procedimiento documentado** con su propia
   rama, su propia batería de pruebas y su propia revisión de parches.
3. El bloque `[patch.crates-io]` es **obligatorio y versionado junto a la revisión**
   (ADR-0001 se apoya en el hallazgo de Fase 0: sin los parches, la resolución de
   dependencias falla por la feature `line-dictionaries` de `parley_engine`).
4. Se mantiene el **plan de salida** del ADR-0008.

## Motivos

- **Es nativo de verdad.** Renderizador propio sobre `wgpu`, motor de texto propio
  (Parley), sin WebView. Cumple el requisito de producto nativo sin discusión.
- **Trae los componentes que este producto necesita**, no una caja de juguetes:
  tabla virtualizada con cabecera fija, columnas ordenables y redimensionables;
  `Sidebar`, `SplitView`, `Toolbar`, `WindowTitlebar`, `Dialog`, `Menu`, `Toast`,
  `Combobox`, `TextField`, `Tree`. Construir eso a mano costaría meses.
- **El arnés de pruebas sin ventana es exactamente lo que un laboratorio necesita**:
  reloj virtual, árbol de elementos, entrada simulada y capturas PNG. Permite
  probar el comportamiento de la interfaz en `cargo test`, algo que la mayoría de
  los frameworks de Rust no ofrece.
- **Licencia MIT**, compatible con producto propietario.
- **Actividad diaria** y `dev` que nunca se reescribe: fijar revisión es seguro.

## Consecuencias

**Positivas**

- Producto nativo, rápido y con identidad visual propia.
- Menos componentes que construir; más tiempo para el dominio del laboratorio.
- Pruebas de interfaz deterministas desde el primer día.

**Negativas (asumidas)**

- Dependencia de un framework pre-release: **habrá rupturas de API** entre revisiones.
- Dependencia de un proveedor único, sin vía de contribución.
- Obligación de arrastrar parches de `winit`, `wgpu-hal`, `parley_engine` y
  `accesskit_*`: cada actualización toca cinco revisiones, no una.
- Linux exige dependencias de sistema en CI (`libxkbcommon`, `libwayland`).

**Mitigaciones**

- Revisión fijada + `Cargo.lock` versionado.
- Aislamiento de la interfaz (ADR-0003) y plan de salida (ADR-0008).
- Presupuesto explícito de actualización: una ventana por fase, no «cuando haga falta».

## Alternativas descartadas

| Alternativa | Motivo del descarte |
| --- | --- |
| `egui` | Estética de herramienta interna; alcanzar calidad comercial exige rehacer tablas, tipografía y formularios |
| `iced` | Sin `accesskit` integrado en la rama principal; sin tabla ni shell de escritorio |
| `Slint` | DSL propia y licencia dual GPLv3/comercial: coste y riesgo contractual |
| `GPUI` | API de bajo nivel: habría que construir todos los controles |
| Tauri/Electron | WebView: memoria, arranque y dependencia del motor del sistema; contradice «nativo» sin justificación aprobada |
