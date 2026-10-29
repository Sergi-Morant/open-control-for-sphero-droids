# Protocolo Bluetooth LE de los droides Sphero Star Wars

Documentación obtenida analizando la app oficial *Star Wars Droids* (APK 1.7.2.4) para poder hablar con los droides desde otra plataforma.
Cada dato lleva una etiqueta de fiabilidad:

- **[APK]** leído directamente del APK (constantes, SDK en C# o tablas de datos).
- **[público]** conocimiento documentado públicamente por la comunidad; coincide con lo visto en el APK, pero no se leyó de ahí.
- **[probado]** confirmado funcionando en un R2-D2 real.
- **[sin confirmar]** supuesto razonable; verificar con un droide.

## 1. Familias de protocolo

| Droide | Prefijo BLE | Protocolo |
|---|---|---|
| BB-8 | `BB-` | v1 (clásico de Sphero/Ollie) |
| R2-D2 | `D2-` | v2 |
| BB-9E | `GB-` | v2 |
| R2-Q5 | `Q5-` | v2 |

Prefijos: `ToyPrefix` en `Sphero.Toys` **[APK]**.

## 2. UUIDs

### v2 (R2-D2, BB-9E, R2-Q5)

Cadenas encontradas en `libCore.so` **[APK]**:

| Función | UUID |
|---|---|
| Servicio API | `00010001-574F-4F20-5370-6865726F2121` |
| API (comandos y respuestas) | `00010002-574F-4F20-5370-6865726F2121` |
| Anti-DoS | `00020005-574F-4F20-5370-6865726F2121` **[probado en R2-D2]** |

Los servicios `00020001…00020004` corresponden a la actualización de firmware (DFU) y no se usan.

### v1 (BB-8)

| Función | UUID |
|---|---|
| Anti-DoS | `22BB746F-2BBD-7554-2D6F-726568705327` |
| Potencia de transmisión | `22BB746F-2BB2-7554-2D6F-726568705327` |
| Despertar CPU | `22BB746F-2BBF-7554-2D6F-726568705327` |
| Comandos (escritura) | `22BB746F-2BA1-7554-2D6F-726568705327` |
| Respuestas (notificación) | `22BB746F-2BA6-7554-2D6F-726568705327` |

Los UUIDs existen en `libCore.so` **[APK]**; la asignación de función a cada uno es **[público]** / **[sin confirmar]** en BB-8.

## 3. Conexión

### v2

1. Conectar y descubrir servicios y características.
2. Suscribirse a las notificaciones de `00010002`.
3. Escribir la cadena ASCII `usetheforce...band` en la característica anti-DoS (con respuesta). **[APK]** (cadena en `libCore.so`) **[probado]**.
4. Enviar *Wake* (`0x13/0x0D`) al procesador Bluetooth.

Sin el paso 3 el droide ignora los comandos.

### v1 (BB-8)

1. Suscribirse a las notificaciones de `2BA6`.
2. Escribir `011i3` en anti-DoS (`2BBD`). **[APK]** (cadena en `libCore.so`).
3. Escribir `0x07` en potencia de transmisión (`2BB2`). **[público]**
4. Escribir `0x01` en despertar CPU (`2BBF`). **[público]**
5. Enviar comandos por `2BA1`.

## 4. Paquetes v2

```
8D  flags  [target]  [source]  did  cid  seq  [payload…]  chk  D8
```

- **flags:** `0x01` es respuesta, `0x02` pide respuesta, `0x04` solo error, `0x08` actividad, `0x10` lleva *target*, `0x20` lleva *source*. **[público]**
- **chk:** `~(suma de todos los bytes entre 8D y chk) & 0xFF`. **[público]**, verificado con tests unitarios.
- **Escapado** de todo lo que hay entre `8D` y `D8`: `AB` → `AB 23`, `8D` → `AB 05`, `D8` → `AB 50`. **[público]**
- **Respuestas:** tras `seq` viene un byte de error (`0` = correcto) y después el payload.
- Valores multibyte en *big-endian*. Los `float` son IEEE-754 de 32 bits *big-endian*. **[APK]** (ver `Array.Reverse` en las clases `Command`)

### Direcciones de destino: pendiente

El droide tiene dos procesadores: **Nordic** (Bluetooth, energía, información del sistema) y **ST** (movimiento, animaciones, audio, LEDs, sensores).
El byte que identifica cada uno y la presencia de *source* los construye código nativo y **no se pudieron leer del APK**.

La app prueba al conectar cuatro esquemas enviando un *Ping* (`0x10/0x00`) al ST y se queda con el primero que responda sin error:

| # | Nordic | ST | Con *source* |
|---|---|---|---|
| 1 | `0x11` | `0x12` | sí |
| 2 | `0x01` | `0x02` | sí |
| 3 | `0x01` | `0x02` | no |
| 4 | `0x11` | `0x12` | no |

El resultado se guarda y se muestra en Ajustes › Direcciones y en el log. Si lo averiguas en tu droide, compártelo en un *issue*.

## 5. Comandos v2

IDs tomados de las clases `Sphero.Commands.*` del SDK en C# (`Assembly-CSharp-firstpass.dll`) **[APK]**.
Lista completa en [`tools/sdk_commands.txt`](../tools/sdk_commands.txt) (formato `Comando did cid`, en decimal).

> **Aviso:** la tabla de símbolos nativa (`tools/ids.txt`) a veces difiere del SDK en C# (por ejemplo, *EnableIdleAnimations* es `0x2C` en el SDK y `0x0A` en `libCore.so`). Esta app usa los valores del SDK en C#, que son los que envía la app.

| Función | Destino | did | cid | Payload |
|---|---|---|---|---|
| Ping | ST | `0x10` | `0x00` | — |
| Despertar | Nordic | `0x13` | `0x0D` | — |
| Dormir | Nordic | `0x13` | `0x01` | — |
| Sueño profundo | Nordic | `0x13` | `0x00` | — |
| Estado de batería | Nordic | `0x13` | `0x04` | respuesta: estado (`0` cargado, `1` cargando, `2` sin cargar, `3` normal, `4` baja, `5` crítica) |
| Voltaje de batería | Nordic | `0x13` | `0x03` | respuesta: `u16` (se interpreta como centésimas de voltio, **[sin confirmar]**) |
| Notificar batería | Nordic | `0x13` | `0x05` | `bool` |
| Versión Nordic / principal | Nordic | `0x11` | `0x00` / `0x17` | respuesta: 3 × `u16` (mayor, menor, revisión) |
| Renombrar | Nordic | `0x19` | `0x03` | nombre ASCII |
| Conducir con rumbo | ST | `0x16` | `0x07` | `u8` velocidad, `u16` rumbo 0-359, `u8` flags (`1` marcha atrás) |
| Fijar «adelante» (reset yaw) | ST | `0x16` | `0x06` | — |
| Estabilización | ST | `0x16` | `0x0C` | `u8` (`0` ninguna, `1` completa) |
| Reproducir animación | ST | `0x17` | `0x05` | `u16` índice |
| Parar animación | ST | `0x17` | `0x2B` | — |
| Posición de la cúpula | ST | `0x17` | `0x0F` | `float` grados (-160…180) |
| Acción de patas | ST | `0x17` | `0x0D` | `u8`: `0` parar, `1` tres patas, `2` dos patas, `3` contoneo |
| Cúpula sigue al movimiento | ST | `0x17` | `0x29` | `u8` |
| Animaciones de reposo | ST | `0x17` | `0x2C` | `bool` |
| Modo trofeo | ST | `0x17` | `0x2D` | `bool` |
| Interacción droide a droide | ST | `0x17` | `0x2F` | `bool` |
| Reproducir sonido | ST | `0x1A` | `0x07` | `u16` sector, `u8` modo (`0` inmediato, `1` solo si no suena, `2` tras el actual) |
| Volumen | ST | `0x1A` | `0x08` | `u8` |
| Parar audio | ST | `0x1A` | `0x0A` | — |
| LEDs | ST | `0x1A` | `0x0E` | `u16` máscara de LEDs + un `u8` de brillo por cada bit activo, en orden ascendente |
| Animación de LEDs en reposo | ST | `0x1A` | `0x19` | — |
| Configurar colisiones | ST | `0x18` | `0x11` | `u8` método (`1`), `xUmbral`, `xVelocidad`, `yUmbral`, `yVelocidad`, `tiempoMuerto` |
| Activar aviso de colisión | ST | `0x18` | `0x14` | `u8` **[sin confirmar]** (de la tabla nativa) |

### Notificaciones asíncronas v2

| Evento | did | cid | Payload |
|---|---|---|---|
| Colisión | `0x18` | `0x12` | aceleración XYZ (`i16` ×3), flags, potencia XY (`i16` ×2), velocidad, tiempo |
| Cambio de batería | `0x13` | `0x06` | estado |
| Animación terminada | `0x17` | `0x11` | `u16` índice |
| Acción de patas terminada | `0x17` | `0x26` | `u8` acción |

### Máscaras de LEDs

| Droide | Bit → LED |
|---|---|
| R2-D2 / R2-Q5 | `0x01` frontal R, `0x02` frontal G, `0x04` frontal B, `0x08` pantalla lógica, `0x10` trasero R, `0x20` trasero G, `0x40` trasero B, `0x80` holoproyector |
| BB-9E | `0x01` cuerpo R, `0x02` cuerpo G, `0x04` cuerpo B, `0x08` apuntado, `0x10` cabeza |

Fuente: enums `R2D2ToyLed`, `R2Q5ToyLed`, `Bb9EToyLed` **[APK]**.

## 6. Paquetes y comandos v1 (BB-8)

```
FF  FF  did  cid  seq  dlen  [payload…]  chk
```

`dlen = len(payload) + 1`. `chk = ~(did + cid + seq + dlen + Σ payload) & 0xFF`. Sin escapado.
Respuesta: `FF FF mrsp seq dlen [payload…] chk`. Asíncronos: `FF FE id dlenHi dlenLo [payload…] chk`.
Formato leído en `V1Session` del SDK Java **[APK]**.

| Función | did | cid | Payload |
|---|---|---|---|
| Rodar | `0x02` | `0x30` | `u8` velocidad, `u16` rumbo, `u8` estado (`0` parar, `1` ir, `2` calibrar), `u8` marcha atrás |
| Fijar rumbo | `0x02` | `0x01` | `u16` |
| Estabilización | `0x02` | `0x02` | `u8` |
| LED principal | `0x02` | `0x20` | R, G, B, `0` |
| LED trasero | `0x02` | `0x21` | brillo |
| Impulso | `0x02` | `0x31` | `u8` tiempo (décimas de s), `u16` rumbo |
| Configurar colisiones | `0x02` | `0x12` | igual que en v2 |
| Estado de energía | `0x00` | `0x20` | respuesta: versión, estado (`1` cargando, `2` normal, `3` baja, `4` crítica), voltaje `u16` (centésimas de V), … |
| Notificar energía | `0x00` | `0x21` | `u8` |
| Dormir | `0x00` | `0x22` | `u16` 0, `u8` 0, `u16` 0 |

Los IDs de `did/cid` son **[APK]** (`V1SpheroCommandId`, `V1CoreCommandId` y las clases `Command`). Asíncronos: colisión `0x07`, energía `0x01` **[público]**.

## 7. Catálogos de sonidos y animaciones

Los sonidos se reproducen enviando su **«Sector»** (`DroidSoundDefinitions.csv`, columna `Sector`) con el comando `0x1A/0x07`. En el APK la llamada es `PlayAudioFile((ushort)Sector, PlayImmediately)` **[APK]** y el audio está en la memoria del propio droide.

Las animaciones se lanzan con su **`AnimationID`** (`RobotAnimatorAnimationDefinitions.csv`) en `0x17/0x05`; la columna `Time` da su duración en segundos. Los índices son independientes para cada tipo de droide.

El R2-Q5 no tiene tabla de sonidos propia en el APK; la app reutiliza la del R2-D2 **[sin confirmar]**.

`tools/gen_catalog.py` regenera `Catalog.swift` a partir de esos CSV (rutas dentro del APK: `assets/CMS/StaticData/`).

## 8. Reproducir el análisis

```sh
unzip app.apk -d apk_extract
brew install jadx dotnet && dotnet tool install -g ilspycmd

# SDK en C# (payloads y IDs de comando)
ilspycmd -p -o cs_firstpass apk_extract/assets/bin/Data/Managed/Assembly-CSharp-firstpass.dll

# Constantes nativas
python3 -m venv venv && venv/bin/pip install pyelftools
venv/bin/python tools/dump_ids.py apk_extract/lib/armeabi-v7a/libCore.so

# Catálogo de sonidos y animaciones
python3 tools/gen_catalog.py apk_extract/assets/CMS/StaticData R2Control/Sources/Catalog.swift
```

## 9. Por confirmar

- Bytes de destino y flags de v2 (ver §4).
- Unidad del voltaje de batería.
- Asignación de función de las características v1 de BB-8 y funcionamiento completo de BB-8, BB-9E y R2-Q5.
- Lista de sonidos del R2-Q5.
- `0x18/0x14` (aviso de colisión) en v2.
