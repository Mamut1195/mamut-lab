# ADR-0002 · Persistencia local: SQLite con `rusqlite`

- **Estado:** aceptado
- **Fecha:** 2026-11-18

## Contexto

El producto es **offline-first**: un archivo de base de datos local por instalación,
con adjuntos en disco, copias de seguridad y migraciones. No hay servidor. Un solo
usuario por instalación, con posibilidad de varios equipos compartiendo una carpeta
de red en el futuro (no en la primera versión).

El stack preferido menciona **SQLx u otra alternativa justificada**.

## Decisión

Se usa **SQLite** con **`rusqlite` 0.43** (feature `bundled`), acceso **síncrono**,
un único escritor y un `Connection` por hilo lector.

- Repositorios con una implementación concreta en `lab-storage` (hoy `mamut-storage`).
  La abstracción en *traits* se introduce cuando exista un segundo adaptador (la
  sincronización futura); construirla antes sería abstracción especulativa.
- Migraciones versionadas en SQL, aplicadas en una transacción, con prueba de
  migración por versión.
- `PRAGMA journal_mode = WAL`, `foreign_keys = ON`, `busy_timeout`.
- Adjuntos como archivos en `data/attachments/<sha256>/…` referenciados desde la base
  de datos, nunca como BLOB (salvo miniaturas).

## Motivos

- **SQLx está pensado para asincronía y para bases de datos de servidor.** Su gran
  ventaja —consulta verificada en compilación— exige una base de datos viva en tiempo
  de compilación o un caché `.sqlx` versionado; para una app de escritorio con SQLite
  local eso añade un paso frágil de build sin resolver un problema que tengamos.
- **Traer un runtime asíncrono (tokio) a una app de escritorio** por un motor de
  almacenamiento local es complejidad sin retorno: el volumen de datos es pequeño y
  las operaciones son cortas.
- **`bundled`** compila SQLite dentro del binario: cero dependencia de la DLL del
  sistema, cero variación entre máquinas. Verificado en Fase 0 sobre Windows.
- **`rusqlite` es la base de facto del ecosistema** (la usan también herramientas de
  migración), con mantenimiento activo (0.43.0 publicada el 2026-11-15).
- Los **traits de repositorio** permiten dobles en memoria para pruebas y un futuro
  adaptador de sincronización, sin cambiar el dominio.

## Consecuencias

**Positivas**

- Un solo archivo portable; copia de seguridad con `VACUUM INTO` o la API de backup.
- Sin runtime asíncrono en el hilo de interfaz.
- Pruebas de persistencia contra `:memory:` rápidas y deterministas.

**Negativas**

- Las consultas no se verifican en compilación: se compensa con **pruebas de
  integración por repositorio** y con un conjunto de consultas centralizado.
- Todo acceso a datos es bloqueante: debe hacerse **fuera del hilo de interfaz**
  (hilo de almacenamiento dedicado + canal de mensajes hacia la UI).

## Reglas de datos

1. **Nunca se sobrescribe un resultado.** Cada cálculo produce una nueva fila con
   `calculated_at`, `engine_version` y `inputs_hash`; la anterior queda como histórica.
2. **Los datos originales son inmutables.** Corregir una lectura crea una nueva
   versión con su motivo y su autor.
3. **Unidades explícitas en columna**: `value` + `unit` en tablas separadas o en
   columnas adyacentes; nunca un número «pelado» cuyo significado dependa del contexto.
4. **Instantes en UTC** (`TEXT` ISO-8601) y conversión solo en la capa de
   presentación; zona horaria del laboratorio configurable (`America/Guayaquil` por
   defecto).
5. **Toda tabla de negocio lleva `created_at`, `updated_at` y `revision`.**

## Alternativas descartadas

| Alternativa | Motivo |
| --- | --- |
| SQLx | Asincronía y verificación en compilación que exige BD viva o caché: coste mayor que el beneficio para SQLite local |
| Diesel | Excelente tipado, pero macros y esquema acoplado; el SQL explícito es más auditable para cálculos normativos |
| `sled` / clave-valor | Sin consultas relacionales ni integridad referencial: inadecuado para trazabilidad |
| Archivos JSON | Sin transacciones, sin índices, sin concurrencia segura |
