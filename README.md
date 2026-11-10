# Open control for Sphero Droids

Apps de código abierto para controlar por Bluetooth LE los droides **R2-D2, R2-Q5, BB-9E y BB-8** de Sphero:

- **iPhone** (`ios/`): SwiftUI + CoreBluetooth.
- **Ordenador** (`desktop/`): Rust, para **macOS, Linux y Windows**.

Nació porque la app oficial *Star Wars Droids* solo se encuentra para Android y quería controlar mi R2-D2 desde iOS.

> **Proyecto no oficial e independiente.** No está afiliado, respaldado ni patrocinado por Sphero, Disney ni Lucasfilm.
> *Sphero*, *R2-D2*, *BB-8*, *BB-9E*, *R2-Q5* y *Star Wars* son marcas de sus respectivos propietarios y se citan solo para indicar con qué droides es compatible.
> No se incluye código, audio, firmware ni recursos de la app original: solo una reimplementación propia del protocolo Bluetooth LE.

## Estado

| Parte | Estado |
|---|---|
| iPhone + R2-D2 | Probado por el autor: funciona. El mando y los botones de parada son lo último que se ha añadido y aún no se han probado con un mando real |
| iPhone + R2-Q5, BB-9E, BB-8 | Sin probar (mismo protocolo v2 que el R2-D2, y v1 para BB-8) |
| **macOS, Linux y Windows** | **Sin probar.** Compilan y pasan las pruebas automáticas del protocolo, pero no se han ejecutado con un droide ni con un mando |

Si pruebas algo de lo que falta, abre un *issue* con el log (pestaña **Log › Copiar log**).

## Funciones

Las dos aplicaciones tienen las mismas pestañas, salvo lo indicado:

- **Conectar:** busca los cuatro tipos de droide por el prefijo de su nombre BLE (`BB-`, `D2-`, `GB-`, `Q5-`) y permite tener varios conectados a la vez.
- **Conducir:** joystick, turbo, estabilización, modo apuntado («fijar adelante») y el impulso de BB-8. Cúpula y patas (3 patas, 2 patas, contoneo) en R2-D2 y R2-Q5. En ordenador, sin mando, se elige entre **joystick con el ratón** o **teclado (WASD / flechas)**.
- **Mando:** ver más abajo.
- **Emociones:** las animaciones de cada droide (R2-D2 55, R2-Q5 50, BB-9E 44) con su duración.
- **Automático** (también dentro de Sonidos): «Vamos a ver una película» (emotes de Watch With Me) y «Haz monerías» (emotes de Emociones) al azar cada N segundos, con opción de incluir o no el Ion Blast.
- **Sonidos:** catálogo del droide con buscador, volumen y charla aleatoria.
- **Seguir:** el droide intenta seguir al dispositivo usando solo la intensidad de la señal Bluetooth: avanza en zigzag y corrige el rumbo hacia donde crece la señal (aproximado).
- **Patrulla, Dibujar y conducir, Luces, Energía, Ajustes, Log.**
- **Parar todo:** botón siempre visible, más un botón por cada actividad en marcha.
- Nombres de sonidos y animaciones **en español con el original entre paréntesis**, p. ej. `Acceso a paneles (R2_Access Panels)`.
- Solo iPhone: **órdenes de voz** con el micrófono del teléfono («vamos a ver una película», «haz monerías», «detente», «sígueme»…) y opción de seguir activo con la pantalla bloqueada.

### Mando

Si hay un mando conectado (Xbox, PlayStation…, por USB o Bluetooth) se usa automáticamente:

| Control | Acción |
|---|---|
| Stick izquierdo | Mueve el droide |
| Stick derecho (eje X) | Gira la cúpula; al soltarlo vuelve al centro |
| RT / R2 | Sube la velocidad máxima |
| LT / L2 | Baja la velocidad máxima |
| A / Cruz | 3 patas |
| B / Círculo | 2 patas |
| Y / Triángulo | Contoneo |
| X / Cuadrado | Parar patas |

Todos los botones se pueden reasignar en la pestaña **Mando** (parar todo, fijar «adelante», sonido o emote al azar, activar/apagar los modos automáticos, turbo…). PlayStation sigue la misma estructura porque los botones se identifican por posición.

### Qué no incluye

Actualización de firmware (OTA, con riesgo de dejar el droide inservible), realidad aumentada, tienda, Force Band, sincronía de *Watch With Me* con películas, y las animaciones y sonidos de BB-8 (viven en assets de Unity y en el motor Wwise de la app original). Las órdenes de voz y el modo en segundo plano no están en las apps de ordenador.

## Instalar

### iPhone

Requisitos: Mac con Xcode 15 o superior, [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`) y un iPhone real (el simulador no tiene Bluetooth).

```sh
cd ios
xcodegen generate
open R2Control.xcodeproj
```

En Xcode: *Signing & Capabilities* → elige tu equipo (vale un Apple ID gratuito), **cambia el *Bundle Identifier*** (`com.sergio.r2control` en `project.yml`) y pulsa Run con el iPhone conectado. Con un Apple ID gratuito la app caduca a los 7 días.

> Las *releases* incluyen un `.ipa` **sin firmar**: hay que firmarlo con tu propio Apple ID antes de instalarlo.

### Ordenador

Descarga de la *release* el archivo de tu sistema, o compila:

```sh
cd desktop
cargo run --release
```

| Sistema | Archivo | Notas |
|---|---|---|
| macOS (Intel y Apple Silicon) | `open-control-for-sphero-droids-macos-universal.zip` | App sin notarizar: la primera vez, clic derecho › Abrir. macOS pedirá permiso de Bluetooth |
| Linux x86_64 | `open-control-for-sphero-droids-linux-x86_64.tar.gz` | Necesita BlueZ (`bluetoothd` activo) |
| Windows x86_64 | `open-control-for-sphero-droids-windows-x86_64.zip` | Windows 10 o superior con Bluetooth LE |

### Steam Deck

En modo escritorio, abre una terminal (Konsole) y ejecuta:

```sh
curl -fsSL https://raw.githubusercontent.com/Sergi-Morant/open-control-for-sphero-droids/main/desktop/packaging/steamdeck/install.sh | bash
```

No necesita `sudo` ni modifica el sistema. Instala en tu carpeta personal, crea un acceso en el menú y, con Steam abierto, la añade a tu biblioteca (sección «No de Steam») para abrirla desde el modo juego con los controles de la Deck como mando. Si se actualiza, basta con volver a ejecutarlo. Para quitarla: `install.sh --uninstall`. **Sin probar** en una Deck real.

Para compilar en Linux hacen falta `libdbus-1-dev`, `libudev-dev`, `pkg-config` y las librerías de X11/Wayland (ver `.github/workflows/desktop.yml`).

Despierta al droide sacándolo del cargador y pulsa **Buscar droides**.

> **R2-D2:** pulsa **3 patas** antes de moverlo.

## Estructura

```
ios/        app de iPhone (SwiftUI)         project.yml → XcodeGen
desktop/    app de ordenador (Rust)         btleplug + egui + gilrs
docs/       PROTOCOLO.md: el protocolo Bluetooth LE documentado
tools/      scripts para extraer datos del APK (IDs de comando, catálogo de sonidos y animaciones)
```

## Cómo se hizo

Es una reimplementación del protocolo, no un port. Se analizó el APK de la app oficial (versión 1.7.2.4) solo para averiguar cómo habla con el droide: la app es de Unity con Mono, así que los IDs de comando y los payloads se leyeron de `Assembly-CSharp-firstpass.dll` (con `ilspycmd`) y de la tabla de símbolos de `libCore.so` (`tools/dump_ids.py`); los UUIDs y el secreto anti-DoS, de las cadenas de `libCore.so`; y el número de cada sonido y animación, de las tablas CSV de la app (`tools/gen_catalog.py`).

Para regenerar los datos necesitas tu propio APK (no se distribuye aquí). Detalles en [`docs/PROTOCOLO.md`](docs/PROTOCOLO.md).

## Aviso legal

- **Sin material de terceros:** el repositorio no contiene el APK de la app oficial, ni su código, audio, imágenes, animaciones ni firmware. Solo hay código propio y datos de interoperabilidad (identificadores de comando, UUIDs y tablas de números de sonidos y animaciones).
- **Interoperabilidad:** el protocolo se averiguó analizando la app para que droides que ya posees funcionen con otros programas. Revisa la legislación de tu país sobre ingeniería inversa antes de analizar un APK por tu cuenta.
- **Tu responsabilidad:** úsalo con tus propios droides y bajo tu responsabilidad. La actualización de firmware no está incluida a propósito.
- **Licencias de terceros:** el escritorio incluye librerías de código abierto; sus avisos de licencia están en [`desktop/THIRD-PARTY-LICENSES.txt`](desktop/THIRD-PARTY-LICENSES.txt) y se incluyen en cada archivo descargable.
- **Titulares de derechos:** si crees que algo de este repositorio infringe tus derechos, abre un *issue* y se revisará.

## Licencia

[MIT](LICENSE)
