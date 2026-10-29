# R2 Control — app iOS no oficial para droides Sphero Star Wars

App para iPhone (SwiftUI + CoreBluetooth) que controla los droides **R2-D2, R2-Q5, BB-9E y BB-8** de Sphero.
Nació porque la app oficial *Star Wars Droids* solo se encuentra para Android y quería controlar mi R2-D2 desde iOS.

> **Proyecto no oficial.** No está afiliado, respaldado ni patrocinado por Sphero, Disney ni Lucasfilm.
> *Sphero*, *R2-D2*, *BB-8*, *BB-9E*, *R2-Q5* y *Star Wars* son marcas de sus respectivos propietarios.
> No se incluye código, audio, firmware ni recursos de la app original: solo una reimplementación del protocolo Bluetooth LE.

## Estado

| Droide | Estado |
|---|---|
| R2-D2 | Probado por el autor en un iPhone: parece funcionar bien |
| R2-Q5, BB-9E | Sin probar (mismo protocolo v2 que el R2-D2) |
| BB-8 | Sin probar (protocolo v1, implementado a partir del SDK de la app original) |

Si pruebas alguno de los que faltan, abre un *issue* con el log (pestaña **Log › Copiar log**).

## Funciones

- **Conexión:** busca los cuatro tipos de droide por el prefijo de su nombre BLE (`BB-`, `D2-`, `GB-`, `Q5-`) y permite tener varios conectados a la vez.
- **Conducir:** joystick con límite de velocidad, turbo, marcha atrás, estabilización, modo apuntado («fijar adelante») y el impulso de BB-8. Cúpula y patas (3 patas, 2 patas, contoneo) en R2-D2 y R2-Q5.
- **Emociones:** las animaciones de cada droide (R2-D2 55, R2-Q5 50, BB-9E 44) con su duración.
- **Automático:** lanza al azar los emotes de «Reacciones (Watch With Me)» o de «Emociones» cada N segundos (3 a 120), con opción de incluir o no el Ion Blast y botón de iniciar/apagar.
- **Sonidos:** catálogo del droide con buscador, volumen y charla aleatoria.
- **Patrulla:** el droide avanza solo, retrocede y gira al chocar, lanza alarmas y deja un registro de eventos.
- **Dibujar:** dibujas un recorrido en pantalla y el droide lo sigue (por tiempo; requiere calibración).
- **Luces:** LEDs propios de cada modelo.
- **Ajustes:** batería, versiones de firmware, modos autónomos (reposo, otros droides, modo trofeo), reacción a golpes, renombrar, dormir/despertar.
- **Log:** paquetes enviados y recibidos, con botón para copiarlo.

### Qué no incluye

Actualización de firmware (OTA, con riesgo de dejar el droide inservible), realidad aumentada, tienda, Force Band, sincronía de *Watch With Me* con películas, y las animaciones y sonidos de BB-8 (viven en assets de Unity y en el motor Wwise de la app original).

## Compilar e instalar

Requisitos: Mac con Xcode 15 o superior, [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`) y un iPhone real (el simulador no tiene Bluetooth).

```sh
cd R2Control
xcodegen generate
open R2Control.xcodeproj
```

En Xcode: *Signing & Capabilities* → elige tu equipo (vale un Apple ID gratuito), **cambia el *Bundle Identifier*** (`com.sergio.r2control` en `project.yml`) por uno tuyo y pulsa Run con el iPhone conectado.

Para usarla, despierta al droide sacándolo del cargador y pulsa **Buscar droides**.

> **R2-D2:** pulsa **3 patas** en la pestaña Conducir antes de moverlo.

## Estructura

```
R2Control/
  project.yml            definición del proyecto (XcodeGen)
  Sources/
    SpheroPacket.swift   paquetes v1 y v2: codificación, escapado, checksum, decodificadores
    Droid.swift          un droide: conexión BLE, handshake, detección de direcciones, comandos
    DroidManager.swift   búsqueda y conexiones múltiples
    DroidKind.swift      tipos de droide, luces y prefijos
    Controllers.swift    patrulla y «dibujar y conducir»
    Catalog.swift        sonidos y animaciones (generado, ver tools/)
    *View.swift          interfaz
tools/
  dump_ids.py            extrae IDs de comando de la tabla de símbolos de libCore.so
  gen_catalog.py         genera Catalog.swift desde las tablas CSV de la app original
  ids.txt                IDs extraídos de libCore.so
  sdk_commands.txt       dispositivo/comando de cada orden del SDK en C#
docs/
  PROTOCOLO.md           protocolo Bluetooth LE documentado
```

## Cómo se hizo

Es una reimplementación del protocolo, no un port. Se analizó el APK de la app oficial (versión 1.7.2.4) solo para averiguar cómo habla con el droide:

1. **Estructura:** es una app de Unity con Mono, así que la lógica está en `Assembly-CSharp-firstpass.dll` (SDK de Sphero) y `libCore.so`.
2. **Decompilación:** `jadx` para la capa Java e `ilspycmd` para los ensamblados .NET.
3. **UUIDs, secretos y constantes:** de las cadenas y la tabla de símbolos de `libCore.so` (`tools/dump_ids.py`).
4. **Payloads:** leídos en las clases `Command` del SDK en C#.
5. **Catálogos:** las tablas CSV de la app (`DroidSoundDefinitions`, `RobotAnimatorAnimationDefinitions`) dan el número de cada sonido y animación (`tools/gen_catalog.py`).

Para regenerar los datos necesitas tu propio APK (no se distribuye aquí). Detalles en [`docs/PROTOCOLO.md`](docs/PROTOCOLO.md).

## Qué falta por confirmar

En el R2-D2 el direccionamiento v2 es `Nordic 0x01 / ST 0x02`, sin *source*. En BB-9E y R2-Q5 la app prueba cuatro variantes al conectar y guarda la que el droide acepta; si los pruebas, comparte tu log (`✓ direcciones detectadas: …`).

Nota: el R2-D2 rechaza conducir (error «ocupado») mientras reproduce una animación; la app la cancela al mover el joystick.

## Aviso

Úsalo bajo tu responsabilidad y con tus propios droides. Revisa la legislación de tu país sobre ingeniería inversa para interoperabilidad antes de analizar un APK.

## Herramientas utilizadas

Parte del análisis y del desarrollo se hizo con ayuda de herramientas de Anthropic.

## Licencia

[MIT](LICENSE)
