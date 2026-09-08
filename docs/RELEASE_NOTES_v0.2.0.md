# 🚀 CastScreen v0.2.0 — "Dark Studio Edition"

¡Nos complace anunciar el lanzamiento de **CastScreen v0.2.0**! Esta versión representa una evolución completa de la experiencia de usuario, incorporando una nueva interfaz gráfica profesional **Dark Studio**, un instalador de Windows nativo con Inno Setup, control de instancia única y bandeja del sistema, y mejoras significativas en el rendimiento y la sincronización de audio/video.

---

## 📦 Descarga del Instalador

| Archivo | Plataforma | Descripción |
| :--- | :--- | :--- |
| **[`CastScreen_Setup.exe`](https://github.com/gabjesus15/CastScreen/releases/download/v0.2.0/CastScreen_Setup.exe)** | Windows 10 / 11 (x64) | Instalador completo con lanzador universal, emisor y receptor |

---

## ✨ Novedades y Características Destacadas

### 🎨 1. Nueva Interfaz Gráfica "Dark Studio"
- **Paleta de color profesional:** Diseñada específicamente para creadores de contenido y streamers con tonos profundos Obsidian (`#0B0D13`, `#131722`), acentos dinámicos Cyan Stream (`#00F2FE`), Indigo Glow (`#4FACFE`) y estados de emisión en vivo Coral (`#FF4B6E`).
- **HUD flotante en el Receptor:** Overlay no invasivo en la ventana de previsualización que muestra métricas en tiempo real (FPS, bitrate en kbps, latencia SRT calculada, paquetes perdidos y recuperados).
- **Vúmetros estéreo de alta precisión:** Indicadores de volumen estéreo por aplicación y mezcla maestra con respuesta visual instantánea y colores de advertencia pre-clipping.

### 🚀 2. Lanzador Unificado (`CastScreen.exe`)
- Si ejecutas el acceso directo principal, ahora se abre un **Lanzador Universal** con dos tarjetas interactivas:
  - **Emisor (PC Gaming):** Captura VRAM DirectX 11, codificación NVENC y mezclador de audio por aplicación.
  - **Receptor (Laptop Stream):** Previsualización en vivo a 60 FPS con baja latencia para TikTok Live Studio y OBS Studio.
- Selector de modo intuitivo que recuerda la configuración preferida del equipo.

### 🛡️ 3. Instancia Única y Minimizado a la Bandeja (System Tray)
- **Named Mutex de Windows:** Impide que se inicien accidentalmente dos instancias simultáneas que compitan por los puertos SRT (9000) o la sesión de DXGI. Si ya está ejecutándose, restaura la ventana activa al primer plano.
- **Icono en la bandeja del sistema:** Permite minimizar la aplicación mientras transmites tu partida sin ocupar espacio en la barra de tareas.

### 🎛️ 4. Mezclador de Audio por Aplicación Nativo
- Controla el volumen o silencia aplicaciones de forma individual (`IAudioSessionManager2`):
  - ¿Quieres escuchar a tus amigos en Discord mientras juegas pero no quieres que salgan en el stream? Desactiva Discord con un solo clic.
  - No requiere instalar VoiceMeeter, VB-Cable ni controladores virtuales que introduzcan latencia.

### ⚡ 5. Sincronización A/V Matemática Absoluta (Wi-Fi 6 Ready)
- Basado en el reloj monotónico de hardware de Windows (`QueryPerformanceCounter` a ~10 MHz).
- Multiplexación en paquetes **MPEG-TS** con Presentation Time Stamps (**PTS de 90 kHz**) idénticos para video NVENC y audio AAC.
- Búfer de resiliencia **SRT de 1.000 ms** que garantiza cero desincronización y cero crujidos incluso en conexiones Wi-Fi con fluctuaciones de señal.

### 🛠️ 6. Instalador Nativo de Windows (`CastScreen_Setup.exe`)
- Creado con **Inno Setup** y empaquetado directamente en GitHub Actions.
- Soporte en **Español** e **Inglés**.
- Crea accesos directos en el Menú Inicio y en el Escritorio.
- Proceso de desinstalación limpio registrado en Windows.

---

## 🔧 Cambios Técnicos y Correcciones (Changelog)

- **`castscreen-core`:**
  - Implementación de `SingleInstanceMutex` con soporte para Windows 11 (`CreateMutexW` / `FindWindowW`).
  - Módulo `tray` para control de iconos en la bandeja con `Shell_NotifyIconW`.
  - Módulo `theme` unificado con paleta Obsidian y tokens de diseño para `egui`.
- **`castscreen-launcher`:**
  - Nueva crate agregada al workspace para arranque modular y selector visual.
- **`castscreen-sender`:**
  - Optimización del bucle de interfaz gráfica (`gui.rs`) y panel de control de stream.
  - Limpieza de advertencias de compilación y optimización de tipos de stroke en UI.
- **`castscreen-receiver`:**
  - Restauración y estabilización del HUD flotante con control de volumen horizontal e integración con estadísticas del socket SRT.
- **CI / CD (.github/workflows):**
  - Workflow de release automatizado (`release.yml`) que compila los binarios en modo release optimizado con LTO y empaqueta el instalador con Inno Setup al publicar tags `v*`.

---

## 📋 Requisitos del Sistema

### PC Gaming (Emisor)
- **SO:** Windows 10 o Windows 11 (64 bits).
- **GPU:** NVIDIA GeForce GTX 10-series o superior con soporte NVENC (Recomendado: RTX 2000 / 3000 / 4000).
- **Red:** Conexión Ethernet o Wi-Fi 5 / Wi-Fi 6 a la red local.

### Laptop de Streaming (Receptor)
- **SO:** Windows 10 o Windows 11 (64 bits).
- **CPU / iGPU:** Cualquier procesador con gráficos integrados modernos (Intel UHD/Iris Xe, AMD Radeon Vega/600M).
- **Software de streaming:** [TikTok Live Studio](https://www.tiktok.com/studio/download) o [OBS Studio](https://obsproject.com/).

---

## 👥 Créditos y Agradecimientos
Desarrollado con ❤️ por [@gabjesus15](https://github.com/gabjesus15) y la comunidad de CastScreen.
