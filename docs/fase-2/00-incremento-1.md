# Fase 2 · Incremento 1 — Alta de muestras

**Fecha:** 2026-11-21 · **Estado:** completado y probado

## Alcance

Primer incremento de la fase 2: **registrar una muestra desde la aplicación, validarla,
guardarla y verla en la tabla**, con su entrada de auditoría. Es el primer circuito real
de usuario del producto.

## Qué se ha implementado

| Pieza | Dónde | Qué hace |
| --- | --- | --- |
| `Store` | `crates/mamut-app/src/store.rs` | Puerta única de escritura: abre/migra la base, guarda la muestra y su auditoría, cuenta muestras y ensayos |
| `SampleForm` | `crates/mamut-app/src/state.rs` | Borrador con los campos como **texto** (no se convierte mientras se teclea) |
| `AppState::save_form` | `crates/mamut-app/src/state.rs` | Valida con los constructores del dominio y guarda; si falla, el error se muestra y **no se escribe nada** |
| Sección Muestras | `crates/mamut-app/src/view.rs` | Botón «+ Nueva muestra», formulario de 6 campos, selector de material, Guardar/Cancelar |
| Contadores reales | `AppState::reload` | Muestras y ensayos salen de la base, no de constantes |
| Auditoría | `Store::create_sample` | Cada alta deja una entrada `created` con actor y momento |

## Decisiones

- **Validación en el dominio, no en la vista.** El formulario construye `SampleCode`,
  `CivilDate`, `Actor` y `Sample`; cada uno rechaza lo suyo y el mensaje se muestra tal cual.
  No hay una segunda copia de las reglas en la interfaz.
- **El formulario ocupa el área de trabajo** mientras está abierto. Con la tabla visible a
  la vez, el layout flex comprimía el formulario y los botones quedaban fuera del alcance
  del puntero (detectado por las pruebas, no a ojo).
- **Escritura en el hilo de la interfaz** por ahora: un `INSERT` local es instantáneo y el
  usuario está esperando el resultado. Queda como deuda DT-19, no como olvido.
- **Operador local** (`local`) con el nombre que se escribe en el formulario: la auditoría
  registra *quién* recepcionó, aunque todavía no haya gestión de usuarios.

## Evidencia

```
cargo test --workspace   →  284 pruebas, 0 fallos
cargo clippy -D warnings →  0
cargo fmt --check        →  0
```

Pruebas nuevas que cubren el circuito completo:

- `Store`: guardar deja la muestra consultable **y auditada**; código duplicado falla;
  el límite de la tabla no recorta el total; sin ensayos el contador es cero.
- `AppState`: guardar una muestra válida la persiste y limpia el formulario; código vacío,
  fecha imposible, cliente vacío y código duplicado **no guardan nada** y explican por qué;
  cancelar no escribe; el material elegido llega a la tabla; sin almacenamiento avisa en
  vez de fingir.
- Armazón (headless, con clics simulados): alta de punta a punta, error visible en pantalla,
  cancelación, selección de material y `Esc` cerrando el formulario.

Captura: [`fase-2-alta-formulario.png`](fase-2-alta-formulario.png), generada con
`mamut-lab --snap --scene alta`.

## Lo que este incremento todavía no hace

- No edita ni anula muestras; solo da de alta.
- No adjunta fotos ni documentos (siguiente incremento).
- No hay ensayo de compresión en la interfaz todavía: el cálculo existe y está probado,
  pero aún no se captura desde pantalla.
- No hay informe PDF.

## Siguiente incremento

1. Adjuntos (fotos/documentos) con almacén por contenido y su auditoría.
2. Ensayo de compresión de cilindros de punta a punta: espécimen, mediciones, cálculo,
   resultado persistido y conformidad (que seguirá sin dictaminar hasta que el pack tenga
   una norma verificada).
3. Informe PDF con `printpdf`.
