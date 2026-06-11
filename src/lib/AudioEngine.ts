import { toolManager } from './ToolManager.svelte.ts';

export class AudioEngine {
  private ctx: AudioContext | null = null;
  private buffers: Record<string, AudioBuffer> = {};

  constructor() {
    // SSR 환경(Node.js) 방어
    if (typeof window !== 'undefined') {
      this.ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
      this.preloadSounds();
    }
  }

  private async preloadSounds() {
    // 사운드 파일 매핑 (번개 사운드 추가됨)
    const soundFiles = {
      here: '/sounds/point_02.mp3',
      ding: '/sounds/bell_01.mp3',
      clap: '/sounds/claps_01.mp3',
      lightning: '/sounds/Thunder_01.mp3',
      fireworks: '/sounds/fire_01.mp3',
      rainbow: '/sounds/rainbow_01.mp3'
    };

    for (const [key, src] of Object.entries(soundFiles)) {
      try {
        const response = await fetch(src);
        const arrayBuffer = await response.arrayBuffer();
        if (this.ctx) {
          // AudioBuffer로 완벽한 메모리 디코딩 (한 번만 로드됨)
          const audioBuffer = await this.ctx.decodeAudioData(arrayBuffer);
          this.buffers[key] = audioBuffer;
        }
      } catch (e) {
        console.error(`[AudioEngine] Failed to preload ${key}:`, e);
      }
    }
  }

  private playSound(key: string) {
    if (toolManager.isMuted || !this.ctx || !this.buffers[key]) return;
    
    // 브라우저 정책으로 인해 Suspended 상태일 경우 깨우기
    if (this.ctx.state === 'suspended') {
      this.ctx.resume();
    }

    // 연타(Overlapping) 재생 핵심 로직:
    // 매번 새로운 AudioBufferSourceNode를 생성하여 버퍼를 연결합니다.
    // 재생이 끝나면 가비지 컬렉터(GC)에 의해 자동으로 메모리에서 해제되므로 누수가 전혀 발생하지 않습니다.
    const source = this.ctx.createBufferSource();
    source.buffer = this.buffers[key];
    source.connect(this.ctx.destination);
    source.start(0);
  }

  public playHere() { this.playSound('here'); }
  public playDing() { this.playSound('ding'); }
  public playClap() { this.playSound('clap'); }
  public playLightning() { this.playSound('lightning'); }
  public playFireworks() { this.playSound('fireworks'); }
  public playRainbow() { this.playSound('rainbow'); }
}

export const audioEngine = new AudioEngine();
