# Guía de Configuración: Recibir CastScreen en TikTok Live Studio

Esta guía explica cómo utilizar **CastScreen Receiver** para previsualizar el juego en tu laptop y emitirlo directamente en **TikTok Live Studio** sin necesidad de abrir OBS.

---

## 1. El Flujo de Trabajo con Previsualización Directa

```
[ PC Gaming: CastScreen Sender ] 
            │ (SRT Wi-Fi 6 Búfer 1000ms)
            ▼
[ Laptop: CastScreen Receiver (Ventana de Previsualización) ]
            │ (Captura de Ventana + Audio de Sistema)
            ▼
[ TikTok Live Studio (Directo a la Audiencia) ]
```

---

## 2. Paso a Paso

### Paso 1: Iniciar y verificar en la Laptop
1. Abre **CastScreen Receiver** en tu Laptop.
2. Ingresa la IP de tu PC Gaming y haz clic en **Conectar**.
3. Verás la pantalla de tu juego a 60 FPS fluidos y escucharás el audio en los altavoces o auriculares de la laptop.
4. **Verificación Pre-Directo:** Comprueba en el vúmetro inferior que el audio llega limpio, sin distorsión y en sincronía con lo que ves en pantalla.

### Paso 2: Agregar la fuente en TikTok Live Studio
1. Abre **TikTok Live Studio** en la laptop.
2. En la barra de fuentes, haz clic en **Añadir fuente** $\rightarrow$ **Capturar ventana**.
3. En la lista de ventanas abiertas, selecciona:
   ```
   [CastScreen Preview]
   ```
4. Ajusta el lienzo a la resolución deseada (Horizontal o Vertical/Móvil).

### Paso 3: Configurar el Audio en TikTok Live Studio
1. En el panel de **Ajustes de Audio** de TikTok Live Studio:
   - Asegúrate de que **Audio del sistema (Altavoces)** esté activo. Como CastScreen Receiver reproduce el audio directamente en la laptop, TikTok Live Studio lo capturará con calidad cristalina.
   - Si vas a hablar con un micrófono conectado a la laptop, agrégalo en la sección de **Micrófono**.

¡Listo! Ya puedes hacer clic en **Transmitir en Vivo**.
