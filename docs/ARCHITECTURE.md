# Arquitectura Técnica Exhaustiva de CastScreen

Documento de referencia para desarrolladores y colaboradores del proyecto **CastScreen**.

---

## 1. Diagrama de Flujo de Datos End-to-End

```mermaid
flowchart TD
    subgraph PC_Gaming["PC GAMING (RTX 3070 + i5 11400)"]
        subgraph Capture_ZeroCopy["Captura de Video Zero-Copy"]
            DXGI["IDXGIOutputDuplication"] -->|ID3D11Texture2D| VRAM["Textura en VRAM"]
            VRAM -->|D3D11 Surface Mapping| NVENC["NVIDIA NVENC (Ampere)"]
        end

        subgraph Audio_Engine["Motor de Audio Multisesión"]
            WASAPI["WASAPI Loopback (48 kHz)"] --> SMIX["PCM Submixer f32"]
            SESSIONS["IAudioSessionManager2 (Apps)"] --> SMIX
            MIC["Micrófono"] --> SMIX
            SMIX --> AAC["AAC-LC Encoder (320 kbps)"]
        end

        subgraph Clock_Sync["Reloj Maestro Monotónico (QPC)"]
            QPC["QueryPerformanceCounter @ 10MHz"]
            QPC -.->|PTS 90kHz| NVENC
            QPC -.->|PTS 90kHz| AAC
        end

        NVENC -->|H.264 NAL Units| MUX["MPEG-TS Muxer (188 Bytes)"]
        AAC -->|ADTS AAC Frames| MUX
        MUX --> SRT_TX["SRT Sender Socket (Búfer 1000ms)"]
    end

    SRT_TX ==>|Red Local Wi-Fi 6 (UDP/SRT)| SRT_RX

    subgraph Laptop_Stream["LAPTOP STREAM (Ryzen 5 5500 + Wi-Fi 6)"]
        SRT_RX["SRT Receiver Socket (Jitter Buffer ARQ)"] --> DEMUX["MPEG-TS Demuxer"]
        DEMUX --> HW_DEC["Decodificador Hardware Radeon"]
        DEMUX --> AUD_DEC["Decodificador Audio AAC"]
        
        HW_DEC --> PREVIEW["Superficie de Previsualización (60 FPS)"]
        AUD_DEC --> SOUND["Reproducción WASAPI + Vúmetros"]
        
        PREVIEW --> APPS["TikTok Live Studio / OBS Studio"]
    end
```

---

## 2. Puntos Clave de Implementación

### 2.1. Cero Copias de Video (Zero-Copy)
- En la PC Gaming, `IDXGIOutputDuplication::AcquireNextFrame` produce una textura `ID3D11Texture2D` residente en la memoria física de la RTX 3070.
- La textura se vincula directamente a NVENC sin invocar `CopyResource` a la memoria RAM de la CPU ni pasar por buffers intermedios en el espacio de usuario.

### 2.2. Reloj Maestro y Sincronización PTS
- El audio y el video no usan tiempos locales independientes.
- Ambos derivan sus marcas de tiempo de `QueryPerformanceCounter` escaladas a 90 kHz:
  $$\text{PTS} = \frac{(T - T_0) \times 90000}{\text{Frequency}_{\text{QPC}}} \pmod{2^{33}}$$
- El multiplexor MPEG-TS intercala los paquetes TS de video y audio con marcas de tiempo relativas idénticas, garantizando sincronismo permanente independientemente del número de horas que dure la transmisión.

### 2.3. Resiliencia Wi-Fi 6 con Búfer SRT
- Wi-Fi 6 es rápido pero sujeto a fluctuaciones momentáneas (*jitter*).
- Al establecer un búfer de 1.000 ms, el emisor SRT puede retransmitir paquetes perdidos hasta 10 veces antes de que el receptor agote su búfer de reproducción.
