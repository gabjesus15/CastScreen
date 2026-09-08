# Guía de Configuración: Recibir CastScreen en OBS Studio

Esta guía explica cómo recibir la señal de video y audio transmitida desde tu PC Gaming directamente en **OBS Studio** instalado en tu Laptop, sin software intermediario.

---

## 1. Requisitos Previos
- **CastScreen Sender** ejecutándose en tu PC Gaming en el puerto `9000`.
- Ambas computadoras conectadas a la misma red local (LAN / Wi-Fi 6).
- Conocer la dirección IP local de tu PC Gaming (ejemplo: `192.168.1.55`).

---

## 2. Configurar la Fuente en OBS Studio (Laptop)

1. Abre **OBS Studio** en tu laptop.
2. En el panel de **Fuentes (Sources)**, haz clic en el botón **`+`** y selecciona **Fuente multimedia (Media Source)**.
3. Asígnale un nombre descriptivo, por ejemplo: `CastScreen Gaming PC`.
4. En la ventana de configuración:
   - **Desmarca** la casilla *"Archivo local"*.
   - En el campo **Entrada (Input)**, escribe:
     ```
     srt://192.168.1.55:9000?mode=caller&latency=1000000
     ```
     *(Sustituye `192.168.1.55` por la IP real de tu PC Gaming. `latency=1000000` equivale a 1.000 ms de búfer para absorber fluctuaciones de Wi-Fi).*
   - En el campo **Formato de entrada (Input Format)**, escribe:
     ```
     mpegts
     ```
   - **Marca** la casilla *"Usar decodificación por hardware si está disponible"*.
5. Haz clic en **Aceptar**.

---

## 3. Configuración de Audio en OBS

1. En el **Mezclador de Audio** de OBS, verás la pista `CastScreen Gaming PC`.
2. Haz clic en el engranaje de configuración de la pista y selecciona **Propiedades de audio avanzadas**.
3. En la fila de `CastScreen Gaming PC`, ajusta **Monitorización de audio**:
   - Selecciona: **Solo monitorización** o **Monitorización y salida** (si deseas escuchar el juego en los audífonos conectados a la laptop).

---

## 4. Pasar la señal de OBS a TikTok Live Studio

Si también transmites en TikTok Live Studio:
1. En OBS Studio, haz clic en **Iniciar Cámara Virtual**.
2. En TikTok Live Studio, añade una fuente de **Cámara** y selecciona **OBS Virtual Camera**.
3. Para el audio, selecciona como fuente de micrófono el dispositivo de monitoreo de OBS o tu cable virtual.
