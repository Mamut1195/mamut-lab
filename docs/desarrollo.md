# Manual de desarrollo · MAMUT Lab

## Requisitos

- Rust **1.95+** (`rustup default stable`).
- Windows: Build Tools de Visual Studio (target `x86_64-pc-windows-msvc`).
- Linux: `libxkbcommon-dev`, `libwayland-dev` (las exige Rune para compilar).
- Conexión a Internet **solo** la primera vez (dependencias). La aplicación funciona sin
  conexión.

## Puesta en marcha

```powershell
cargo test --workspace          # todo
cargo run -p mamut-app          # abre la aplicación
```

`.cargo/config.toml` ya configura `git-fetch-with-cli`, sin lo cual `cargo` se queda
colgado descargando Rune (hallazgo H-2, `docs/fase-0/02-validacion-stack.md`).

## Mapa del workspace

| Crate | Responsabilidad | Puede depender de |
| --- | --- | --- |
| `mamut-core` | Ids, tiempo, unidades, errores, auditoría, huellas | nada del producto |
| `mamut-standards` | Normas, estados de revisión, conformidad, *norm pack* | `core` |
| `mamut-domain` | Entidades e invariantes | `core`, `standards` |
| `mamut-calculations` | Algoritmos puros | `core`, `standards` |
| `mamut-validation` | Plausibilidad de entradas | `core`, `calculations` |
| `mamut-charts` | Geometría de gráficos | **nada** |
| `mamut-storage` | SQLite, migraciones, repositorios | `core`, `domain`, `standards` |
| `mamut-reports` | Composición de informes | `core`, `standards` |
| `mamut-equipment` | Equipos y calibraciones | `core` |
| `mamut-import` / `mamut-export` | CSV/XLSX | `core`, `domain` |
| `mamut-app` | **Interfaz Rune** y composición | todo |

Las reglas se comprueban en `cargo test -p mamut-app --test architecture`. Si añades una
dependencia prohibida, la prueba falla: no es una convención, es un test.

## Reglas que no se negocian

1. **Solo `mamut-app` conoce Rune.** Si necesitas un tipo de Rune en otro crate, el diseño
   está mal: crea un tipo propio y adáptalo en la frontera.
2. **El dominio no hace I/O.** El tiempo se inyecta (`Clock`), los identificadores se
   generan dentro, y nada lee archivos.
3. **Nada se calcula en la vista.** La vista presenta estado ya calculado.
4. **Unidades tipadas.** Un `f64` no cruza una frontera de módulo: usa `Length`, `Mass`,
   `Force`, `Stress`, `Area`.
5. **Sin `unwrap()` en rutas de usuario.** Devuelve error. En pruebas está permitido
   (`clippy.toml`).
6. **No se sobrescribe un resultado.** La base lo impide con disparadores.
7. **No se emite conformidad sin norma revisada y especificación aplicable.** Lo impide el
   tipo `Conformity` y el cargador del pack.

## Añadir un ensayo nuevo

1. `mamut-calculations`: módulo con `Input` (tipado), `Outcome` y una función pura
   `calculate`. Documenta la procedencia de cada fórmula.
2. `mamut-validation`: reglas de plausibilidad, con códigos estables (`E-...`, `W-...`).
3. `mamut-standards`: entrada en el *norm pack* (`docs/normas/packs/`), con su estado.
4. `mamut-storage`: si necesita tablas, nueva migración `V00N__...sql`. **No** edites una
   migración ya aplicada.
5. `mamut-app`: vista del ensayo y cableado.
6. Pruebas: valores de referencia con procedencia, casos límite y casos de rechazo.

## Añadir o actualizar una norma

Ver `docs/normas/README.md`. Resumen:

- Metadatos (designación, año, título, URL y fecha de consulta) → `PendingReview`.
- El ingeniero responsable revisa el documento y completa cláusulas y límites.
- Solo entonces se marca `Verified` con nombre y fecha.
- **Nunca** se copia el texto de la norma al repositorio.

El pack se embebe en el binario (`include_str!` en `mamut-app/src/main.rs`), así que
actualizarlo requiere recompilar **el pack**, no la lógica.

## Actualizar Rune (procedimiento)

1. Rama propia: `chore/rune-<rev-corto>`.
2. Cambia **las siete apariciones** de `rev = "..."` en `Cargo.toml` (2 dependencias de
   Rune + 5 parches) a la misma revisión nueva. La prueba
   `rune_y_sus_parches_comparten_la_misma_revision` falla si se desincronizan.
3. `cargo update -p rune` y compila.
4. `cargo test --workspace`.
5. Revisa visualmente las pantallas clave (`cargo run -p mamut-app`).
6. Registra en `docs/deuda-tecnica.md` cualquier cambio de comportamiento.

Si falla y no hay solución en un día, se vuelve a la revisión anterior y se anota el motivo.

## Migraciones de base de datos

- Un archivo por versión, en `crates/mamut-storage/migrations/`.
- Se registra en `MIGRATIONS` (`crates/mamut-storage/src/lib.rs`).
- Se aplican en una transacción; una migración que falla no deja la base a medias.
- **Prohibido editar una migración ya publicada**: se añade otra.
- Cada migración necesita una prueba de que se aplica desde cero y de que es idempotente.

## Pruebas

| Tipo | Dónde | Qué prueba |
| --- | --- | --- |
| Unidad | `src/**/mod tests` | Lógica pura, casos límite |
| Integración | `crates/*/tests/` | Contratos públicos, persistencia |
| Interfaz | `crates/mamut-app/tests/shell.rs` | Armazón sin ventana, atajos, navegación |
| Arquitectura | `crates/mamut-app/tests/architecture.rs` | Reglas de dependencia |

```powershell
cargo test --workspace
cargo test -p mamut-app --test shell
cargo test -p mamut-app --test architecture
```

## Formato y lint

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

La CI ejecuta ambos en los tres sistemas operativos.
