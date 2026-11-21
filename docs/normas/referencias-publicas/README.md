# Referencias públicas para implementar los ensayos

Esto **no son las normas**. Son procedimientos publicados por administraciones públicas de
Estados Unidos (departamentos de transporte estatales y el Cuerpo de Ingenieros), de acceso
libre, que describen en detalle los mismos ensayos que las normas ASTM/AASHTO de referencia.

## Para qué sirven y para qué no

| Sirven para | **No** sirven para |
| --- | --- |
| Implementar la matemática y el procedimiento | Afirmar que el software cumple ASTM o AASHTO |
| Escribir pruebas con valores y pasos reales | Sustituir la copia licenciada del laboratorio |
| Detectar diferencias entre variantes (ver CBR/LBR) | Emitir un dictamen de conformidad |

Regla que se aplica a todo lo que hay aquí: **se implementa, se marca `PendingReview`, y el
ingeniero del laboratorio cliente verifica con su copia de la norma.** El producto nunca
presenta estas referencias como normativas.

## Índice

| Ensayo | Documento | Edición | Cobertura | Dossier |
| --- | --- | --- | --- | --- |
| Granulometría por tamices | TxDOT **Tex-110-E** | abril 2024 | Completa (fórmulas y redondeo) | [granulometria.md](granulometria.md) |
| Límite líquido | TxDOT **Tex-104-E** | abril 2024 | Completa (multipunto y un punto) | [limites-atterberg.md](limites-atterberg.md) |
| Límite plástico | TxDOT **Tex-105-E** | — | Completa | [limites-atterberg.md](limites-atterberg.md) |
| Compactación (Proctor) | TxDOT **Tex-113-E** y **Tex-114-E**, FDOT **FM 1-T 180** | ene 2025 / feb 2005 / may 2017 | Completa (ecuaciones de densidad seca) | [compactacion.md](compactacion.md) |
| Densidad y humedad en sitio | TxDOT **Tex-115-E** | enero 2025 | Descargado, pendiente de analizar | — |
| CBR | FDOT **FM 5-515** | enero 2017 | **Parcial y con reserva**: es LBR, no CBR | [cbr-lbr.md](cbr-lbr.md) |

## Huecos conocidos (no hay fuente pública legítima todavía)

| Ensayo | Qué falta | Por qué |
| --- | --- | --- |
| **Compresión de cilindros (C39/C39M)** | El método completo, incluida la tabla de corrección por esbeltez | En los buscadores aparecen PDFs de ASTM subidos por terceros. **No se han usado**: son reproducciones no autorizadas. La alternativa honesta es comprar una copia o pedir al primer cliente que verifique con la suya |
| **CBR (T 193 / D1883)** | El método con las cargas de referencia correctas | Lo que se encontró es LBR (Florida), que usa 800 psi en vez de 1000/1500 psi |
| **Clasificación SUCS y AASHTO (D2487 / D3282)** | Las tablas de clasificación | Son el corazón de la norma; no aparecen reproducidas en los procedimientos de ensayo |
| **Agregados: C136, C29, C127, C128, C566** | Los métodos | No se han buscado todavía; TxDOT tiene serie propia (Tex-4xx-A) que habría que localizar |

Estos huecos **no bloquean** el desarrollo: bloquean, como mucho, marcar un ensayo como
`Verified`. El producto funciona en modo cálculo mientras tanto.

## Cómo se usó este material

1. Se descargaron los PDF desde las URLs oficiales (las que aparecen en cada dossier).
2. Se extrajo el texto y se localizaron las secciones **CALCULATIONS** de cada método.
3. Se transcribieron las ecuaciones y los criterios de redondeo **tal como están publicados**.
4. Se anotó, en cada dossier, qué parte es trasladable y qué parte hay que confirmar contra
   la norma de referencia.

Los PDF no se guardan en el repositorio: se citan por URL y cualquiera puede reproducir la
descarga. Lo que queda versionado es el **análisis**, que es lo que el código necesita.
