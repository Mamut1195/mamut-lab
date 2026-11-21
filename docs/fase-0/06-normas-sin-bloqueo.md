# Normas: cómo avanzar sin ellas (y sin inventar nada)

> Respuesta a la pregunta: *«¿no puedes sacar las normas de tu memoria o dejarlas para
> el final?»*
>
> **Sí a las dos cosas, con una condición: separar lo que es matemática de lo que es
> norma.** Este documento explica qué se construye ya, qué espera, y por qué el proyecto
> no se detiene.

---

## 1. Respuesta directa sobre «sacarlas de memoria»

Hay tres cosas distintas que la gente llama «la norma», y no se pueden tratar igual:

### Lo que **sí** puedo aportar

- **La matemática de los ensayos.** Tensión = fuerza / área; porcentaje que pasa =
  (masa retenida acumulada / masa total); índice de plasticidad = LL − LP; densidad seca
  = densidad húmeda / (1 + humedad). Son **definiciones de la magnitud**, no opiniones.
  Están en cualquier libro de materiales y no cambian entre ediciones de la norma.
- **La estructura** de cada ensayo: qué se mide, qué se calcula, qué se informa, qué
  unidades se usan, qué validaciones tienen sentido.
- **Un borrador** de la implementación para que un ingeniero lo revise y lo firme.

### Lo que **no** voy a hacer

- Darte de memoria una **tabla de corrección** (por ejemplo, el factor por relación
  altura/diámetro), una **tolerancia**, una **velocidad de carga** o un **criterio de
  aceptación**, y presentarlo como normativo.
- Poner un número de cláusula que no puedo verificar.
- Marcar una norma como revisada sin que un ingeniero la haya revisado.

### Por qué esa línea es importante (y no simple prudencia)

La fórmula casi nunca es el problema. **El problema son los detalles que deciden si un
resultado se acepta o se rechaza.** Un informe que dice «conforme» apoyado en un factor
recordado es indefendible en una auditoría, y el daño no es del software: es del
laboratorio que lo firmó.

Pero fíjate en la consecuencia práctica: **eso afecta solo a la última capa**. Todo lo
demás se puede construir hoy.

---

## 2. La separación que desbloquea el proyecto

| Capa | Ejemplo | ¿Se puede hacer ya? |
| --- | --- | --- |
| **Metadatos** | «ASTM C39/C39M, edición 2026, título oficial» | ✅ Sí, es público y verificable |
| **Matemática** | «σ = P / A» con sus unidades | ✅ Sí, es la definición de la magnitud |
| **Contenido normativo** | corrección por esbeltez, velocidad de carga, criterios de aceptación | ⏳ Espera el documento y la firma de un ingeniero |

Se implementa ya la capa 1 y la 2. La capa 3 entra **al final, como dato**, sin
recompilar el programa. Formalizado en [ADR-0009](../adr/0009-enlace-tardio-normas.md) y
con formato concreto en [`docs/normas`](../normas/README.md).

---

## 3. Qué se puede construir sin normas (esto es la fase 2 completa)

| Funcionalidad | ¿Depende de la norma? |
| --- | --- |
| Registrar muestras con identificador único, procedencia, cliente, proyecto, material | No |
| Adjuntar fotos y documentos con trazabilidad | No |
| Registrar especímenes con dimensiones y masa | No |
| Capturar la carga máxima y calcular la resistencia | No (es la capa 2) |
| Historial completo de ensayos por muestra | No |
| Auditoría: quién cambió qué y cuándo | No |
| Informe PDF profesional con datos originales, cálculos y gráfico | No |
| Exportación CSV/XLSX | No |
| Gestión de equipos y alertas de calibración | No |
| Gráficos técnicos | No |
| **Veredicto «conforme / no conforme»** | **Sí** → se activa al cargar el pack verificado |

Es decir: **el 90 % del producto no necesita la norma.** Lo que necesita la norma es el
dictamen, que es una función adicional, no el cimiento.

---

## 4. Los dos modos del producto

```
MODO CÁLCULO      (disponible desde el primer día)
  · registra, calcula, guarda, informa
  · el informe dice: "Resultado calculado. Sin juicio de conformidad:
    la referencia normativa está pendiente de revisión."
  · trazabilidad completa

MODO CONFORMIDAD  (se activa cuando llega el pack verificado)
  · además compara con la especificación aplicable
  · emite Conforme / No conforme con la especificación citada
  · el informe lo imprime
```

El segundo modo no es otro programa: es la misma aplicación con un dato más.

---

## 5. Lo que tú puedes hacer, y lo que no hace falta que hagas

### No hace falta que hagas nada para empezar

La fase 1 y la fase 2 avanzan completas. No hay ninguna tarea tuya bloqueando el
desarrollo.

### Cuando puedas (tareas pequeñas, sin prisa)

| Tarea | Tiempo | Cuándo importa |
| --- | --- | --- |
| Decirme el nombre del **ingeniero responsable** que firmará las revisiones | 1 minuto | Antes de activar el modo conformidad |
| Confirmar si ya tienes **C39/C39M** en PDF o en papel | 1 minuto | Fase 2, para el dictamen |
| Confirmar la **plataforma** (¿Windows para el primer cliente?) | 1 minuto | Fase 1 |
| Confirmar que la fase 2 es **solo compresión de cilindros** | 1 minuto | Fase 2 |

### Si nunca compras las normas

El producto sigue siendo valioso y vendible: registro, trazabilidad, cálculo
reproducible, informes profesionales e historial. Muchos laboratorios lo compran
precisamente por eso, porque su criterio de aceptación viene del **proyecto**, no de la
norma de ensayo.

---

## 6. Control del riesgo (para que no se convierta en un problema oculto)

1. **Todo lo no verificado se muestra.** La interfaz y el PDF llevan el aviso; no hay
   forma de que un usuario confunda un cálculo con un dictamen.
2. **El pack inicial está casi vacío a propósito.** Solo metadatos públicos y
   `spec_limits: null`. Ver [`pack-inicial.json`](../normas/packs/pack-inicial.json).
3. **La regla es verificable por código.** `conformity_enabled = true` exige norma
   `Verified` y `spec_limits` no nulo; el cargador rechaza lo contrario y hay pruebas
   con packs inválidos a propósito.
4. **Queda registrado en la deuda técnica** (DT-10) con su condición de pago.

---

## 7. Lo que sí te pido entender

No voy a escribir en el software «ASTM C39, cláusula 7.3, factor 0,92» sacado de mi
memoria. Si lo hiciera, el producto parecería más completo y sería **peor**: estaría
poniendo en un informe técnico un dato que nadie puede defender.

Lo que sí voy a hacer es que, el día que tengas el documento y el ingeniero lo revise,
**incorporarlo cueste una tarde de configuración y no un desarrollo**. Eso es lo que
significa «dejar las normas para el final» bien hecho.
