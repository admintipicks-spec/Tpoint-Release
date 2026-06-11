export type ToolType = 
  // 설명 도구 (Blue) - 상태 유지
  | 'pointer' | 'magnifier' | 'pen' | 'highlighter' | 'circle' | 'box' | 'text' | 'eraser' 
  // 주목 도구 (Orange) - 즉시 발동
  | 'here' | 'ding' | 'clap' 
  // 화면 효과 (Purple) - 즉시 발동
  | 'lightning' | 'fireworks' | 'rainbow' 
  // 기타 (Green) - 상태 유지(캡처) 및 즉시 발동(전체삭제)
  | 'capture' | 'clear_all' | 'mute';

import { audioEngine } from './AudioEngine';

export interface ToolConfig {
  color: string;
  size: number;
  opacity?: number;
  bold?: boolean;
}

class ToolManager {
  activeTool = $state<ToolType>('pointer');
  activeSettingsPanel = $state<ToolType | null>(null);
  panelPosition = $state<{x: number, y: number}>({x: 0, y: 0});
  isSystemSettingsOpen = $state<boolean>(false);
  isTextInputActive = $state<boolean>(false);
  isMuted = $state<boolean>(false);
  effectTrigger = $state<{ name: ToolType, id: number } | null>(null);
  toastMessage = $state<string | null>(null);
  captureWidgets = $state<{ id: string, x: number, y: number, w: number, h: number, isLocked: boolean }[]>([]);
  toolbarPosition = $state<'top' | 'left' | 'right'>('top');

  configs = $state<Partial<Record<ToolType, ToolConfig>>>({
    pen: { color: '#ff0000', size: 5 },
    highlighter: { color: '#ffff00', size: 20, opacity: 0.5 },
    circle: { color: '#0000ff', size: 5 },
    box: { color: '#00ff00', size: 5 },
    text: { color: '#000000', size: 24, bold: false },
    eraser: { color: '#ffffff', size: 20 },
  });

  setActiveTool(tool: ToolType) {
    const instantTools: ToolType[] = ['lightning', 'fireworks', 'rainbow', 'clear_all'];
    if (instantTools.includes(tool)) {
      this.activeSettingsPanel = null;
      this.effectTrigger = { name: tool, id: Date.now() };
      console.log(`[Action] Triggered instant tool: ${tool}`);
      return;
    }
    this.activeTool = tool;
    this.activeSettingsPanel = null; // 툴이 바뀌면 패널 닫기
  }

  toggleSettingsPanel(tool: ToolType, pos?: {x: number, y: number}) {
    // 설정이 없는 도구 예외 처리
    const noSettingsTools: ToolType[] = ['pointer', 'magnifier', 'here', 'ding', 'clap', 'lightning', 'fireworks', 'rainbow', 'capture', 'clear_all', 'mute'];
    if (noSettingsTools.includes(tool)) return;

    if (this.activeSettingsPanel === tool) {
      this.activeSettingsPanel = null;
    } else {
      this.activeSettingsPanel = tool;
      if (pos) this.panelPosition = pos;
    }
  }

  closeSettingsPanel() {
    this.activeSettingsPanel = null;
  }

  updateConfig(tool: ToolType, config: Partial<ToolConfig>) {
    if (!this.configs[tool]) {
      this.configs[tool] = { color: '#000000', size: 5 };
    }
    this.configs[tool] = { ...this.configs[tool], ...config } as ToolConfig;
  }

  getConfig(tool: ToolType): ToolConfig {
    return this.configs[tool] || { color: '#000000', size: 5 };
  }

  setTextInputActive(isActive: boolean) {
    this.isTextInputActive = isActive;
  }

  toggleMute() {
    this.isMuted = !this.isMuted;
  }

  openSystemSettings() {
    this.isSystemSettingsOpen = true;
  }

  closeSystemSettings() {
    this.isSystemSettingsOpen = false;
  }

  showToast(message: string, durationMs: number = 3000) {
    this.toastMessage = message;
    setTimeout(() => {
      if (this.toastMessage === message) {
        this.toastMessage = null;
      }
    }, durationMs);
  }

  updateCaptureWidget(id: string, x: number, y: number, w: number, h: number, isLocked?: boolean) {
    const idx = this.captureWidgets.findIndex(w => w.id === id);
    if (idx > -1) {
      this.captureWidgets[idx].x = x;
      this.captureWidgets[idx].y = y;
      this.captureWidgets[idx].w = w;
      this.captureWidgets[idx].h = h;
      if (isLocked !== undefined) this.captureWidgets[idx].isLocked = isLocked;
    } else {
      this.captureWidgets.push({ id, x, y, w, h, isLocked: isLocked ?? false });
    }
  }

  async setToolbarPosition(pos: 'top' | 'left' | 'right') {
    this.toolbarPosition = pos;
    const posInt = pos === 'top' ? 0 : pos === 'left' ? 2 : 3;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_toolbar_position', { position: posInt });
    } catch (e) {
      console.error(e);
    }
  }

  removeCaptureWidget(id: string) {
    this.captureWidgets = this.captureWidgets.filter(w => w.id !== id);
  }

  clearCaptureWidgets() {
    this.captureWidgets = [];
  }
}

export const toolManager = new ToolManager();
