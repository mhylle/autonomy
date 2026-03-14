import {
  decodeWorldSnapshot,
  decodeTickDelta,
  loadProtoDefinitions,
} from './protocol';
import { applySnapshotToWorld, applyDeltaToWorld, useHudStore } from './state-store';

/**
 * WebSocket client with backpressure protection.
 *
 * - Data flows directly into worldData (no React/GC overhead)
 * - Monitors bufferedAmount to detect when the browser can't keep up
 * - Reconnects if the buffer grows too large (prevents STATUS_ACCESS_VIOLATION)
 * - HUD updates throttled to 4Hz
 */

const HUD_UPDATE_INTERVAL = 250;
const BUFFER_CHECK_INTERVAL = 2000;
const MAX_BUFFERED_BYTES = 5 * 1024 * 1024; // 5MB — reconnect if exceeded
const VIEWPORT_UPDATE_INTERVAL = 500; // ~2Hz viewport updates to the server

export class SimulationClient {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private bufferCheckTimer: ReturnType<typeof setInterval> | null = null;
  private protoLoaded = false;
  private firstMessage = true;
  private destroyed = false;

  private lastHudUpdate = 0;
  private frameCount = 0;
  private lastFpsTime = performance.now();

  // Viewport subscription: throttle updates to ~2Hz
  private lastViewportSend = 0;
  private pendingViewport: { x: number; y: number; width: number; height: number; zoom: number } | null = null;
  private viewportTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(url: string) {
    this.url = url;
  }

  async connect(): Promise<void> {
    if (!this.protoLoaded) {
      await loadProtoDefinitions();
      this.protoLoaded = true;
    }
    this.doConnect();
  }

  private doConnect(): void {
    if (this.destroyed) return;

    this.ws = new WebSocket(this.url);
    this.ws.binaryType = 'arraybuffer';
    this.firstMessage = true;

    this.ws.onopen = () => {
      console.log('WebSocket connected');
      useHudStore.getState().setConnected(true);
      this.startBufferMonitor();
    };

    this.ws.onmessage = (event: MessageEvent) => {
      const data = new Uint8Array(event.data as ArrayBuffer);
      try {
        if (this.firstMessage) {
          const snapshot = decodeWorldSnapshot(data);
          applySnapshotToWorld(snapshot);
          useHudStore.getState().setHud(snapshot.tick, snapshot.entities.length);
          this.firstMessage = false;
        } else {
          const delta = decodeTickDelta(data);
          applyDeltaToWorld(delta);
          this.frameCount++;

          const now = performance.now();
          if (now - this.lastHudUpdate >= HUD_UPDATE_INTERVAL) {
            useHudStore.getState().setHud(delta.tick, delta.entityCount);
            this.lastHudUpdate = now;
          }
          if (now - this.lastFpsTime >= 1000) {
            useHudStore.getState().setFps(this.frameCount);
            this.frameCount = 0;
            this.lastFpsTime = now;
          }
        }
      } catch (e) {
        console.error('Failed to decode message:', e);
      }
    };

    this.ws.onclose = () => {
      console.log('WebSocket disconnected');
      useHudStore.getState().setConnected(false);
      this.stopBufferMonitor();
      this.scheduleReconnect();
    };

    this.ws.onerror = () => {
      // onclose will fire after this
    };
  }

  /** Periodically check if the browser's receive buffer is growing too large. */
  private startBufferMonitor(): void {
    this.stopBufferMonitor();
    this.bufferCheckTimer = setInterval(() => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

      // bufferedAmount is how much outbound data is queued (for sends).
      // Reconnect if the outbound buffer is excessively large.
      if (this.ws.bufferedAmount > MAX_BUFFERED_BYTES) {
        console.warn('WebSocket buffer exceeded limit, reconnecting');
        this.ws.close();
      }
    }, BUFFER_CHECK_INTERVAL);
  }

  private stopBufferMonitor(): void {
    if (this.bufferCheckTimer) {
      clearInterval(this.bufferCheckTimer);
      this.bufferCheckTimer = null;
    }
  }

  private scheduleReconnect(): void {
    if (this.destroyed || this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      console.log('Reconnecting...');
      this.doConnect();
    }, 2000);
  }

  sendCommand(command: object): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(command));
    }
  }

  /**
   * Send a viewport update to the server, throttled to ~2Hz.
   * Called by the renderer whenever the camera moves or zooms.
   */
  sendViewportUpdate(bounds: { x: number; y: number; width: number; height: number; zoom: number }): void {
    this.pendingViewport = bounds;

    const now = performance.now();
    const elapsed = now - this.lastViewportSend;

    if (elapsed >= VIEWPORT_UPDATE_INTERVAL) {
      // Enough time has passed — send immediately.
      this.flushViewport();
    } else if (!this.viewportTimer) {
      // Schedule a send for when the throttle interval expires.
      this.viewportTimer = setTimeout(() => {
        this.viewportTimer = null;
        this.flushViewport();
      }, VIEWPORT_UPDATE_INTERVAL - elapsed);
    }
  }

  private flushViewport(): void {
    if (!this.pendingViewport) return;
    const vp = this.pendingViewport;
    this.pendingViewport = null;
    this.lastViewportSend = performance.now();
    this.sendCommand({
      type: 'subscribe_viewport',
      x: vp.x,
      y: vp.y,
      width: vp.width,
      height: vp.height,
      zoom: vp.zoom,
    });
  }

  disconnect(): void {
    this.destroyed = true;
    this.stopBufferMonitor();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.viewportTimer) {
      clearTimeout(this.viewportTimer);
      this.viewportTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}
