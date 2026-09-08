# AGENTS.md - Contexto y Guía de Arquitectura para Inteligencia Artificial

Este archivo sirve como el documento maestro de contexto para cualquier modelo de Inteligencia Artificial (Gemini, Claude, Copilot, Cursor, etc.) que trabaje en el mantenimiento, evolución o refactorización del proyecto **CastScreen**.

---

## 1. Visión y Propósito del Proyecto

**CastScreen** es una solución de streaming de doble PC por red local (LAN) diseñada específicamente para streamers y gamers que desean jugar en una PC y emitir desde otra (Laptop/PC secundaria) hacia plataformas como **TikTok Live Studio** y **OBS Studio**.

### Problemas que resuelve de forma definitiva:
1. **Desincronización y cortes de audio en Wi-Fi:** Soluciones previas (como OBS Teleport, NDI o VoiceMeeter) sufren de desincronización acumulativa y crujidos sobre Wi-Fi. CastScreen prioriza **sincronización A/V matemática absoluta y calidad de sonido**, utilizando un **búfer generoso (500 ms a 2.000 ms, recomendado 1.000 ms)** sobre el protocolo **SRT (Secure Reliable Transport)** con multiplexación **MPEG-TS**.
2. **Cero impacto en FPS de la PC de juegos:** Captura directa en VRAM por **DirectX 11 (DXGI Desktop Duplication)** y codificación por hardware mediante **NVIDIA NVENC** (RTX 3070). Cero copias innecesarias a la memoria RAM del CPU.
3. **Mezclador de Audio Integrado por Aplicación:** Detección en tiempo real de todas las aplicaciones de Windows que generan sonido (`IAudioSessionManager2`) con interruptores ON/OFF individuales (ej. omitir Discord del stream mientras juegas sin necesidad de cables virtuales complejos).
4. **Previsualización en vivo en la Laptop (`CastScreen-Receiver`):** Ventana a 60 FPS con vúmetros en tiempo real para verificar el flujo antes de transmitir.

---

## 2. Mapa del Workspace en Rust

El proyecto está organizado como un **Cargo Workspace** modular para máxima escalabilidad y separación estricta de responsabilidades:

```
crates/
├── castscreen-core/       # Tipos compartidos, QPC High-Resolution Clock, matemática de mezcla PCM
├── castscreen-capture/    # Captura DXGI DDA (VRAM) y sesiones de audio WASAPI
├── castscreen-encoder/    # NVIDIA NVENC H.264 hardware pipeline y AAC audio encoder
├── castscreen-network/    # Multiplexor MPEG-TS (PTS/PCR) y sockets SRT (Sender/Receiver)
├── castscreen-receiver/   # Aplicación receptora con Live Preview para la Laptop
└── castscreen-sender/     # Aplicación emisora con GUI moderna y Mezclador para la PC Gaming
```

---

## 3. Especificación de APIs de Bajo Nivel de Windows

### 3.1. Video: DXGI Desktop Duplication (`windows-rs`)
- Ubicado en: `crates/castscreen-capture/src/dxgi.rs`
- Utiliza la interfaz COM `IDXGIOutputDuplication::AcquireNextFrame`.
- Obtiene una textura `ID3D11Texture2D` residente en la VRAM de la GPU.
- Pasa la textura directamente a **NVENC** vía Direct3D11 surface mapping. **NUNCA** descargues la textura a la memoria RAM del sistema con `CopyResource` a menos que sea en un fallback de diagnóstico.

### 3.2. Audio: Detección por Sesión y WASAPI Loopback
- Ubicado en: `crates/castscreen-capture/src/sessions.rs` y `wasapi.rs`
- Utiliza `IAudioSessionManager2` para enumerar las sesiones activas (`IAudioSessionEnumerator`).
- Obtiene el Process ID (`GetProcessId`), nombre del ejecutable y estado de volumen.
- Captura de audio maestro mediante `IAudioClient` en modo `AUDCLNT_STREAMFLAGS_LOOPBACK` en formato PCM flotante de 32 bits a 48.000 Hz estéreo.

### 3.3. Reloj Monotónico y Sincronización PTS
- Ubicado en: `crates/castscreen-core/src/clock.rs`
- Utiliza `QueryPerformanceCounter` (QPC) de Windows (resolución de microsegundos, ~10 MHz).
- Se genera un Presentation Time Stamp (PTS) común para video y audio en la escala estándar de 90 kHz:
  $$\text{PTS} = \frac{(T_{\text{evento}} - T_0) \times 90000}{\text{Frecuencia}_{\text{QPC}}}$$
- Tanto los NAL units de H.264 como los bloques de audio AAC se empaquetan en PES de **MPEG-TS** con este PTS.

### 3.4. Red: Protocolo SRT con Búfer Wi-Fi
- Ubicado en: `crates/castscreen-network/src/srt_sender.rs` y `srt_receiver.rs`
- SRT opera sobre UDP con ARQ (Automatic Repeat reQuest).
- Parámetro clave: `SRTO_LATENCY = 1000` (1.000 ms). Si un paquete se pierde en el Wi-Fi 6 de la laptop, SRT lo retransmite en segundo plano antes de que el receptor lo necesite.

---

## 4. Reglas Críticas para Desarrolladores e IAs

1. **Cero asignaciones dinámicas en el bucle principal de frames:**
   - Pre-asigna buffers circulares (`ringbuf` o buffers fijos).
   - No hagas `clone()` ni `to_vec()` en buffers de texturas o bloques PCM en cada ciclo.
2. **Concurrencia multihilo sin bloqueos (Lock-Free):**
   - La comunicación entre captura, codificación y red debe utilizar canales MPSC de alta velocidad (`crossbeam-channel`).
   - Los hilos de procesamiento nunca deben bloquearse esperando a la interfaz de usuario.
3. **Manejo de Errores Idiomático:**
   - Usa `thiserror` en crates de biblioteca (`core`, `capture`, `encoder`, `network`).
   - Usa `anyhow` únicamente en los binarios de aplicación (`sender`, `receiver`).
4. **Preservar la Sincronización de Audio:**
   - El audio es el reloj maestro conceptual. Si se pierden cuadros de video por alguna razón externa, el reloj de audio **NUNCA** debe pausarse ni reiniciarse.

---

## 5. Comandos de Compilación y Test

- Compilar todo el workspace:
  ```bash
  cargo build --workspace
  ```
- Compilar en modo release optimizado:
  ```bash
  cargo build --workspace --release
  ```
- Ejecutar el Emisor (PC Gaming):
  ```bash
  cargo run -p castscreen-sender --release
  ```
- Ejecutar el Receptor (Laptop):
  ```bash
  cargo run -p castscreen-receiver --release
  ```
- Ejecutar tests unitarios (matemática de audio y reloj PTS):
  ```bash
  cargo test --workspace
  ```
