<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { availableMonitors, currentMonitor, getCurrentWindow, type Monitor } from '@tauri-apps/api/window';
  import { PhysicalPosition } from '@tauri-apps/api/dpi';
  import { 
    MousePointer2, PenTool, Search, Square, Circle, Eraser, 
    Highlighter, Type, Zap, Sparkles, Rainbow, Camera, 
    Trash2, VolumeX, Volume2, Hand, Bell, HandMetal, Settings,
    Lock, Unlock, X, Bold, LayoutGrid, Compass, Puzzle, Clock, Users
  } from '@lucide/svelte';
  import CanvasLayer from '$lib/CanvasLayer.svelte';
  import { toolManager } from '$lib/ToolManager.svelte';
  import type { ToolType } from '$lib/ToolManager.svelte';
  import { audioEngine } from '$lib/AudioEngine.ts';
  import { load } from '@tauri-apps/plugin-store';
  import { getVersion } from '@tauri-apps/api/app';
  import { check } from '@tauri-apps/plugin-updater';

  let canvasLayerRef: ReturnType<typeof CanvasLayer>;

  let isLicensed = $state(false);
  let isCheckingLicense = $state(true);
  let isChangingLicense = $state(false);
  let serialInput = $state('');
  let licenseError = $state('');
  let machineUid = $state('');
  let activeSerialKey = $state('');
  let appVersion = $state('로딩 중...');

  let isOSClickThrough = $derived(
    isLicensed &&
    !isChangingLicense &&
    toolManager.activeTool === 'pointer' && 
    !toolManager.isTextInputActive && 
    !toolManager.activeSettingsPanel &&
    !toolManager.isSystemSettingsOpen
  );
  let isTextInputActive = $derived(toolManager.isTextInputActive);
  let activeTool = $derived(toolManager.activeTool);
  let activeSettingsPanel = $derived(toolManager.activeSettingsPanel);

  // 극한의 방어 테스트: 포인터 모드이면서, 설정 패널도 없고, 텍스트 입력창도 없을 때만 OS 클릭 통과
  $effect(() => {
    console.log(`[OS Pass-through] set_pointer_mode: ${isOSClickThrough}`);
    invoke('set_pointer_mode', { isPointer: isOSClickThrough }).catch(console.error);
  });

  let monitors = $state<Monitor[]>([]);
  let currentMon = $state<Monitor | null>(null);

  onMount(async () => {
    try {
      try {
        appVersion = await getVersion();
      } catch (e) {
        console.error('버전 가져오기 실패:', e);
        appVersion = '알 수 없음';
      }
      
      // 자동 업데이트 백그라운드 체크 로직
      setTimeout(async () => {
        try {
          const update = await check();
          if (update && confirm(`새로운 업데이트(v${update.version})가 있습니다. 지금 설치하시겠습니까?`)) {
            await update.downloadAndInstall();
            alert('업데이트가 완료되었습니다. 앱이 종료됩니다. 다시 실행해 주세요.');
            invoke('force_exit');
          }
        } catch (err) {
          console.error('업데이트 체크 실패:', err);
        }
      }, 3000); // 앱 구동 3초 후 체크하여 초기 구동 속도 지연 방지

      machineUid = await invoke<string>('get_machine_uid');
      const store = await load('store.json', { autoSave: false });
      const token = await store.get<{serial: string}>('license_token');
      if (token && token.serial) {
        activeSerialKey = token.serial;
        isLicensed = true;
      }
    } catch (e) {
      console.error('라이선스 확인 에러:', e);
    } finally {
      isCheckingLicense = false;
    }
  });

  async function handleLicenseSubmit() {
    console.log('verify_license 호출 시도:', serialInput.trim());
    if (!serialInput.trim()) {
      licenseError = '시리얼 키를 입력해 주세요.';
      return;
    }
    licenseError = '인증 서버와 통신 중...';
    
    try {
      // 굳이 프론트엔드에서 주소를 알 필요도 없고, CORS 걱정도 없다.
      const response = await invoke<{ success: boolean; message: string }>('verify_license', { serialInput: serialInput.trim() });
      
      if (response.success) {
        // 로컬 스토리지 토큰 저장 및 라이선스 락 해제 진행
        const store = await load('store.json', { autoSave: false });
        await store.set('license_token', { serial: serialInput.trim() });
        await store.save();
        activeSerialKey = serialInput.trim();
        isLicensed = true;
        isChangingLicense = false;
        licenseError = '';
      } else {
        licenseError = response.message;
      }
    } catch (e) {
      licenseError = typeof e === 'string' ? e : '인증 시스템 백엔드 통신 오류가 발생했습니다.';
      console.error("Rust 커맨드 에러 로그:", e);
    }
  }

  $effect(() => {
    if (toolManager.isSystemSettingsOpen) {
      availableMonitors().then((m: Monitor[]) => monitors = m).catch(console.error);
      currentMonitor().then((m: Monitor | null) => currentMon = m).catch(console.error);
    }
  });

  async function handleMonitorChange(event: Event) {
    const select = event.target as HTMLSelectElement;
    const selectedMon = monitors.find(m => m.name === select.value);
    if (selectedMon) {
      const appWindow = getCurrentWindow();
      await appWindow.unmaximize();
      await new Promise(resolve => setTimeout(resolve, 100)); // OS 트랜지션 딜레이 주입
      await appWindow.setPosition(new PhysicalPosition(selectedMon.position.x, selectedMon.position.y));
      await appWindow.maximize();
      currentMon = selectedMon;
    }
  }

  function handleToolSelect(toolName: ToolType) {
    toolManager.setActiveTool(toolName);
  }

  function handleRightClick(e: MouseEvent, tool: ToolType) {
    e.preventDefault();
    if (toolManager.activeTool !== tool) return;
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    
    let x = 0, y = 0;
    const pos = toolManager.toolbarPosition;
    if (pos === 'top') {
      x = rect.left + rect.width / 2;
      y = rect.bottom + 8;
    } else if (pos === 'left') {
      x = rect.right + 8;
      y = rect.top + rect.height / 2;
    } else if (pos === 'right') {
      x = rect.left - 8;
      y = rect.top + rect.height / 2;
    }
    toolManager.toggleSettingsPanel(tool, { x, y });
  }

  function handleOverlayClick(e: MouseEvent) {
    // 팝업/패널 외부를 클릭하면 패널 닫기
    const target = e.target as HTMLElement;
    if (!target.closest('.dockbar-wrapper') && !target.closest('.dummy-text-input')) {
      if (activeSettingsPanel) toolManager.closeSettingsPanel();
      if (isTextInputActive) toolManager.setTextInputActive(false);
      if (toolManager.isSystemSettingsOpen) toolManager.closeSystemSettings();
    }
  }

  // 쨍한 원색 팔레트 (시인성 극대화)
  const colors = ['#FF0000', '#FF7F00', '#FFFF00', '#00FF00', '#00FFFF', '#0000FF', '#FF00FF', '#000000'];

  let showLightningVideo = $state(false);
  let lastLightningTriggerId = 0;

  $effect(() => {
    const trigger = toolManager.effectTrigger;
    if (trigger && trigger.name === 'lightning' && trigger.id !== lastLightningTriggerId) {
      lastLightningTriggerId = trigger.id;
      audioEngine.playLightning();
      showLightningVideo = true;
    }
  });

  function handleLightningEnded() {
    showLightningVideo = false;
  }
</script>

<main class="overlay-container" role="presentation" oncontextmenu={(e) => e.preventDefault()} onmousedown={handleOverlayClick}>
  
  {#if !isCheckingLicense && (!isLicensed || isChangingLicense)}
    <div class="license-lock-screen">
      <div class="license-modal" onmousedown={(e) => e.stopPropagation()}>
        {#if isChangingLicense}
          <button class="modal-close-btn" aria-label="닫기" onclick={() => { isChangingLicense = false; licenseError = ''; }}>&times;</button>
        {/if}
        <Lock size={48} color="#2563eb" strokeWidth={1.5} />
        <h2>T-Point 라이선스 {isChangingLicense ? '변경' : '인증'}</h2>
        <p>프로그램을 사용하려면 시리얼 키를 입력해 주세요.<br/><small>기기 ID: {machineUid}</small></p>
        <input type="text" bind:value={serialInput} placeholder="시리얼 키 입력 (예: EDU-2026-ABC)" onkeydown={(e) => e.key === 'Enter' && handleLicenseSubmit()} />
        {#if licenseError}
          <div class="error-msg">{licenseError}</div>
        {/if}
        <button class="submit-btn" onclick={handleLicenseSubmit} disabled={licenseError === '인증 서버와 통신 중...'}>인증하기</button>
      </div>
    </div>
  {/if}

  {#if isLicensed}
  <button class="app-exit-btn" aria-label="앱 닫기" title="앱 종료" onclick={() => invoke('force_exit')}>
    <X size={24} strokeWidth={3} />
  </button>

  <div 
    class="canvas-layer"
    style:pointer-events={isOSClickThrough ? 'none' : 'auto'}
    style:background={isOSClickThrough ? 'transparent' : 'rgba(255, 255, 255, 0.01)'}
  >
    <CanvasLayer bind:this={canvasLayerRef} />
  </div>

  {#if showLightningVideo}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video 
      src="/videos/Thunder_01.webm" 
      autoplay 
      class="lightning-video-overlay"
      onended={handleLightningEnded}
    ></video>
  {/if}

  <!-- 캡처 위젯 HTML Overlay 버튼 (자물쇠/닫기) -->
  {#each toolManager.captureWidgets as widget (widget.id)}
    <div class="widget-overlay" 
         style="position: absolute; left: {widget.x + widget.w - 80}px; top: {widget.y + 10}px;"
         onpointerdown={(e) => e.stopPropagation()}>
      <button class="overlay-btn" aria-label="잠금/해제" onclick={() => canvasLayerRef?.toggleLockWidget(widget.id)}>
        {#if widget.isLocked}
          <Lock size={16} />
        {:else}
          <Unlock size={16} />
        {/if}
      </button>
      <button class="overlay-btn delete-btn" aria-label="닫기" onclick={() => canvasLayerRef?.removeWidget(widget.id)}>
        <X size={16} />
      </button>
    </div>
  {/each}

  <!-- Toast 알림 -->
  {#if toolManager.toastMessage}
    <div class="toast-message">
      {toolManager.toastMessage}
    </div>
  {/if}

  <!-- 텍스트 툴 등 실제 입력창은 CanvasLayer.svelte 내부에 동적으로 렌더링되므로 더미는 제거함 -->

  <div class="dockbar-wrapper dock-{toolManager.toolbarPosition}">
    <div class="dockbar {['left', 'right'].includes(toolManager.toolbarPosition) ? 'vertical' : ''}">
      
      <div class="tool-group blue-group">
        <button class="tool-btn {activeTool === 'pointer' ? 'active' : ''}" onclick={() => handleToolSelect('pointer')} oncontextmenu={(e) => handleRightClick(e, 'pointer')}>
          <MousePointer2 size={24} /><span>포인터</span>
        </button>
        <button class="tool-btn {activeTool === 'magnifier' ? 'active' : ''}" onclick={() => handleToolSelect('magnifier')} oncontextmenu={(e) => handleRightClick(e, 'magnifier')}>
          <Search size={24} /><span>돋보기</span>
        </button>
        <button class="tool-btn {activeTool === 'pen' ? 'active' : ''}" onclick={() => handleToolSelect('pen')} oncontextmenu={(e) => handleRightClick(e, 'pen')}>
          <PenTool size={24} /><span>그리기</span>
        </button>
        <button class="tool-btn {activeTool === 'highlighter' ? 'active' : ''}" onclick={() => handleToolSelect('highlighter')} oncontextmenu={(e) => handleRightClick(e, 'highlighter')}>
          <Highlighter size={24} /><span>형광펜</span>
        </button>
        <button class="tool-btn {activeTool === 'circle' ? 'active' : ''}" onclick={() => handleToolSelect('circle')} oncontextmenu={(e) => handleRightClick(e, 'circle')}>
          <Circle size={24} /><span>원</span>
        </button>
        <button class="tool-btn {activeTool === 'box' ? 'active' : ''}" onclick={() => handleToolSelect('box')} oncontextmenu={(e) => handleRightClick(e, 'box')}>
          <Square size={24} /><span>박스</span>
        </button>
        <button class="tool-btn {activeTool === 'text' ? 'active' : ''}" onclick={() => handleToolSelect('text')} oncontextmenu={(e) => handleRightClick(e, 'text')}>
          <Type size={24} /><span>텍스트</span>
        </button>
        <button class="tool-btn {activeTool === 'eraser' ? 'active' : ''}" onclick={() => handleToolSelect('eraser')} oncontextmenu={(e) => handleRightClick(e, 'eraser')}>
          <Eraser size={24} /><span>지우개</span>
        </button>
      </div>

      <div class="divider"></div>

      <div class="tool-group orange-group">
        <button class="tool-btn" onclick={() => handleToolSelect('here')}>
          <Hand size={24} /><span>여기</span>
        </button>
        <button class="tool-btn" onclick={() => handleToolSelect('ding')}>
          <Bell size={24} /><span>띵</span>
        </button>
        <button class="tool-btn" onclick={() => handleToolSelect('clap')}>
          <HandMetal size={24} /><span>박수</span>
        </button>
      </div>

      <div class="divider"></div>

      <div class="tool-group purple-group">
        <button class="tool-btn" onclick={() => handleToolSelect('lightning')}>
          <Zap size={24} /><span>번개</span>
        </button>
        <button class="tool-btn" onclick={() => handleToolSelect('fireworks')}>
          <Sparkles size={24} /><span>폭죽</span>
        </button>
        <button class="tool-btn" onclick={() => handleToolSelect('rainbow')}>
          <Rainbow size={24} /><span>무지개</span>
        </button>
      </div>

      <div class="divider"></div>

      <div class="tool-group green-group">
        <button class="tool-btn {activeTool === 'capture' ? 'active' : ''}" onclick={() => handleToolSelect('capture')} oncontextmenu={(e) => handleRightClick(e, 'capture')}>
          <Camera size={24} /><span>캡처</span>
        </button>
        <button class="tool-btn" onclick={() => handleToolSelect('clear_all')}>
          <Trash2 size={24} /><span>전체삭제</span>
        </button>
        <button class="tool-btn {toolManager.isMuted ? 'active' : ''}" onclick={() => toolManager.toggleMute()}>
          {#if toolManager.isMuted}
            <VolumeX size={24} /><span>음소거됨</span>
          {:else}
            <Volume2 size={24} /><span>소리 켬</span>
          {/if}
        </button>
        <button class="tool-btn" onclick={() => toolManager.openSystemSettings()}>
          <Settings size={24} /><span>설정</span>
        </button>
      </div>

    </div>
  </div>

  <!-- 설정 패널 (데이터 연동 및 동적 위치) -->
  {#if activeSettingsPanel}
    {@const config = toolManager.getConfig(activeSettingsPanel)}
    {@const pos = toolManager.panelPosition}
    {@const posType = toolManager.toolbarPosition}
    {@const transform = 
      posType === 'top' ? 'translateX(-50%)' :
      posType === 'left' ? 'translateY(-50%)' :
      'translate(-100%, -50%)'
    }
    <div class="settings-panel" 
         style="position: fixed; left: {pos.x}px; top: {pos.y}px; transform: {transform}; z-index: 99999; pointer-events: auto;" 
         onpointerdown={(e) => e.stopPropagation()}
         onmousedown={(e) => e.stopPropagation()}
         ontouchstart={(e) => e.stopPropagation()}
         onwheel={(e) => e.stopPropagation()}>
      <div class="panel-header">
        <span class="panel-title">설정</span>
        <button class="close-btn" onclick={() => toolManager.closeSettingsPanel()}>&times;</button>
      </div>
      <div class="panel-body">
        <div class="setting-row">
          <span class="label">{activeSettingsPanel === 'text' ? '크기' : '두께'}</span>
          <div class="size-options">
            <button class="size-btn {config.size <= 5 ? 'active' : ''}" onclick={() => toolManager.updateConfig(activeSettingsPanel, { size: 5 })}>S</button>
            <button class="size-btn {config.size > 5 && config.size <= 10 ? 'active' : ''}" onclick={() => toolManager.updateConfig(activeSettingsPanel, { size: 10 })}>M</button>
            <button class="size-btn {config.size > 10 && config.size <= 20 ? 'active' : ''}" onclick={() => toolManager.updateConfig(activeSettingsPanel, { size: 20 })}>L</button>
            <button class="size-btn {config.size > 20 ? 'active' : ''}" onclick={() => toolManager.updateConfig(activeSettingsPanel, { size: 30 })}>XL</button>
            
            {#if activeSettingsPanel === 'text'}
              <div class="divider-vertical"></div>
              <button class="size-btn {config.bold ? 'active' : ''}" onclick={() => toolManager.updateConfig('text', { bold: !config.bold })} title="굵게">
                <Bold size={16} strokeWidth={4} />
              </button>
            {/if}
          </div>
        </div>
        <div class="setting-row">
          <span class="label">색상</span>
          <div class="color-options">
            {#each colors as color}
              <button class="color-btn {config.color === color ? 'selected' : ''}" style="background: {color};" aria-label="{color} 색상 선택" onclick={() => toolManager.updateConfig(activeSettingsPanel, { color })}></button>
            {/each}
            <!-- 커스텀 RGB 피커 -->
            <input type="color" class="color-picker-btn" value={config.color} oninput={(e) => toolManager.updateConfig(activeSettingsPanel, { color: e.currentTarget.value })} title="사용자 지정 색상" />
          </div>
        </div>
        <!-- 형광펜 등 투명도 지원 도구만 표시 -->
        {#if activeSettingsPanel === 'highlighter'}
          <div class="setting-row">
            <span class="label">투명도</span>
            <input type="range" min="0" max="1" step="0.1" value={config.opacity ?? 0.5} oninput={(e) => toolManager.updateConfig(activeSettingsPanel, { opacity: parseFloat(e.currentTarget.value) })} class="slider" />
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- 시스템 설정 껍데기 모달 (모션브로 스타일 스토어 v2 티저) -->
  {#if toolManager.isSystemSettingsOpen}
    <div class="system-settings-modal" onpointerdown={(e) => e.stopPropagation()} onmousedown={(e) => e.stopPropagation()} ontouchstart={(e) => e.stopPropagation()} onwheel={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <span class="modal-title">생태계 확장 스토어 (설정)</span>
        <button class="close-btn" onclick={() => toolManager.closeSystemSettings()}>&times;</button>
      </div>
      <div class="modal-body store-layout">
        
        <div class="store-sidebar">
          <div class="sidebar-item active"><LayoutGrid size={18}/> 위젯 스토어</div>
          <div class="sidebar-item"><Compass size={18}/> 둘러보기</div>
          <div class="sidebar-item"><Puzzle size={18}/> 내 확장 관리</div>

          <hr style="border:none; border-top: 1px solid #e5e7eb; margin: 8px 0;" />
          
          <div class="sidebar-setting">
            <span class="set-label">모니터 이동</span>
            <select class="set-select" onchange={handleMonitorChange}>
              {#each monitors as monitor}
                <option value={monitor.name} selected={currentMon?.name === monitor.name}>{monitor.name}</option>
              {/each}
            </select>
          </div>
          
          <div class="sidebar-setting">
            <span class="set-label">툴바 도킹</span>
            <select class="set-select" onchange={(e) => toolManager.setToolbarPosition(e.currentTarget.value as 'top'|'left'|'right')}>
              <option value="top" selected={toolManager.toolbarPosition === 'top'}>상단 (Top)</option>
              <option value="left" selected={toolManager.toolbarPosition === 'left'}>좌측 (Left)</option>
              <option value="right" selected={toolManager.toolbarPosition === 'right'}>우측 (Right)</option>
            </select>
          </div>

          <div class="sidebar-setting license-setting">
            <span class="set-label">라이선스 관리</span>
            <div class="license-info-box">
              <span class="current-key">{activeSerialKey || '인증 없음'}</span>
              <button class="change-license-btn" onclick={() => { serialInput = ''; licenseError = ''; isChangingLicense = true; }}>변경</button>
            </div>
          </div>

          <div class="sidebar-footer">
            <div class="version-info" style="font-weight: 600; color: #3b82f6; margin-bottom: 6px;">현재 버전: v{appVersion}</div>
            ⓒ 송성근(쏭쌤) X 서명훈<br/>
            All Rights Reserved. (2026.06.01)<br/>
            자유로운 교육용 배포를 허용합니다. 단, 무단 복제 및 수정 후 재배포는 법적으로 금지됩니다.
          </div>
        </div>
        
        <div class="store-content">
          <div class="store-grid">
            <div class="store-card">
              <div class="card-thumb" style="background: #eff6ff;"><PenTool size={32} color="#2563eb" /></div>
              <div class="card-info">
                <h4>판서 위젯 PRO</h4>
                <p>더욱 강력한 판서 전용 도구팩</p>
              </div>
            </div>
            <div class="store-card">
              <div class="card-thumb" style="background: #fff7ed;"><Clock size={32} color="#ea580c" /></div>
              <div class="card-info">
                <h4>수업 타이머</h4>
                <p>팝업형 시계 및 카운트다운</p>
              </div>
            </div>
            <div class="store-card">
              <div class="card-thumb" style="background: #f0fdf4;"><Users size={32} color="#16a34a" /></div>
              <div class="card-info">
                <h4>출석 체크 보드</h4>
                <p>학생 참여도 실시간 집계 위젯</p>
              </div>
            </div>
            <div class="store-card">
              <div class="card-thumb" style="background: #faf5ff;"><Zap size={32} color="#9333ea" /></div>
              <div class="card-info">
                <h4>집중 모드</h4>
                <p>화면 암전 및 스포트라이트 조명</p>
              </div>
            </div>
          </div>

          <!-- Teaser Overlay (블러 처리) -->
          <div class="store-overlay">
            <Lock size={48} color="#4b5563" strokeWidth={1.5} />
            <h2 class="coming-soon-text">업데이트 준비 중 (Coming Soon)</h2>
            <p>v2 릴리스부터 <strong>Tpoint 앱 생태계 스토어</strong>가 오픈됩니다.</p>
          </div>
        </div>

      </div>
    </div>
  {/if}
  {/if}

</main>

<style>
  :global(body) {
    background: transparent !important;
    overflow: hidden;
    margin: 0;
  }

  .overlay-container {
    width: 100vw;
    height: 100vh;
    position: relative;
    /* 최상위 컨테이너는 클릭 이벤트를 낚아채지 않게 none 처리 (하위에서 auto 설정) */
    pointer-events: none;
  }

  .license-lock-screen {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(16px);
    z-index: 999999;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: auto; /* 터치 및 마우스 이벤트 활성화 */
  }

  .license-modal {
    position: relative;
    background: white;
    padding: 40px;
    border-radius: 16px;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
    text-align: center;
    width: 420px;
    border: 1px solid #e5e7eb;
  }

  .modal-close-btn {
    position: absolute;
    top: 12px;
    right: 12px;
    background: none;
    border: none;
    font-size: 24px;
    color: #9ca3af;
    cursor: pointer;
    line-height: 1;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .modal-close-btn:hover {
    background: #f3f4f6;
    color: #ef4444;
  }

  .license-modal h2 {
    margin: 16px 0 8px;
    font-size: 22px;
    font-weight: 800;
    color: #1f2937;
  }

  .license-modal p {
    font-size: 14px;
    color: #6b7280;
    margin-bottom: 24px;
    line-height: 1.5;
  }

  .license-modal small {
    color: #9ca3af;
    font-size: 11px;
    display: block;
    margin-top: 4px;
  }

  .license-modal input {
    width: 100%;
    padding: 12px 16px;
    border: 2px solid #e5e7eb;
    border-radius: 8px;
    font-size: 15px;
    margin-bottom: 16px;
    outline: none;
    transition: border-color 0.2s;
    box-sizing: border-box;
    text-align: center;
  }

  .license-modal input:focus {
    border-color: #3b82f6;
  }

  .license-modal .error-msg {
    color: #ef4444;
    font-size: 13px;
    margin-bottom: 16px;
    font-weight: 500;
  }

  .license-modal button {
    width: 100%;
    padding: 12px;
    background: #2563eb;
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s;
  }

  .license-modal button:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .license-modal button:disabled {
    background: #9ca3af;
    cursor: not-allowed;
  }

  .canvas-layer {
    position: absolute;
    inset: 0;
    z-index: 10;
  }

  /* 위젯 HTML Overlay 버튼 */
  .widget-overlay {
    display: flex;
    gap: 6px;
    z-index: 10001;
    pointer-events: auto;
  }
  .overlay-btn {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: white;
    border: 1px solid #e5e7eb;
    box-shadow: 0 4px 12px rgba(0,0,0,0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: #4b5563;
    transition: all 0.2s;
  }
  .overlay-btn:hover {
    background: #f3f4f6;
  }
  .overlay-btn.delete-btn:hover {
    color: #ef4444;
    background: #fef2f2;
    border-color: #fca5a5;
  }

  /* Toast 메시지 */
  .toast-message {
    position: absolute;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    background: #ef4444;
    color: white;
    padding: 12px 24px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    box-shadow: 0 10px 25px rgba(239, 68, 68, 0.3);
    z-index: 10002;
    pointer-events: none;
    animation: slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translate(-50%, 20px); }
    to { opacity: 1; transform: translate(-50%, 0); }
  }

  /* 시스템 설정 모달 (모션브로 스타일 확장) */
  .system-settings-modal {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 760px;
    height: 540px;
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0,0,0,0.2);
    display: flex;
    flex-direction: column;
    z-index: 100000; /* 툴바, 캔버스보다 무조건 위 */
    pointer-events: auto; /* 바탕화면 통과 방지 */
  }
  .system-settings-modal .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    background: #f9fafb;
    border-bottom: 1px solid #e5e7eb;
    border-top-left-radius: 12px;
    border-top-right-radius: 12px;
  }
  .system-settings-modal .modal-title {
    font-weight: 700;
    font-size: 16px;
    color: #1f2937;
  }
  .system-settings-modal .modal-body {
    flex: 1;
    display: flex;
    overflow: hidden;
    background: #ffffff;
    border-bottom-left-radius: 12px;
    border-bottom-right-radius: 12px;
  }

  /* 스토어 레이아웃 */
  .store-layout {
    display: flex;
    width: 100%;
    height: 100%;
  }

  .store-sidebar {
    width: 200px;
    background: #f3f4f6;
    border-right: 1px solid #e5e7eb;
    padding: 16px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
  }
  
  .sidebar-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-radius: 8px;
    color: #4b5563;
    font-size: 14px;
    font-weight: 500;
    cursor: default; /* Dummy라서 클릭 비활성 */
  }
  .sidebar-item.active {
    background: #e5e7eb;
    color: #111827;
    font-weight: 600;
  }

  .sidebar-setting {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .set-label {
    font-size: 12px;
    font-weight: 600;
    color: #4b5563;
  }
  .set-select {
    padding: 6px;
    border-radius: 6px;
    border: 1px solid #d1d5db;
    font-size: 13px;
    background: white;
    cursor: pointer;
    color: #1f2937;
  }
  
  .license-setting {
    margin-top: 4px;
    padding-top: 12px;
    border-top: 1px solid #e5e7eb;
  }
  
  .license-info-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #ffffff;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    padding: 4px 6px;
    gap: 6px;
  }
  
  .current-key {
    font-size: 11px;
    color: #111827;
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }
  
  .change-license-btn {
    font-size: 11px;
    padding: 4px 8px;
    background: #f3f4f6;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    cursor: pointer;
    color: #374151;
  }
  .change-license-btn:hover {
    background: #e5e7eb;
  }

  .sidebar-footer {
    margin-top: auto;
    font-size: 10.5px;
    color: #9ca3af;
    text-align: center;
    padding: 12px 8px;
    line-height: 1.5;
    word-break: keep-all;
  }

  .store-content {
    flex: 1;
    position: relative;
    background: #ffffff;
    padding: 24px;
  }

  .store-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 20px;
  }

  .store-card {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px;
    border: 1px solid #e5e7eb;
    border-radius: 12px;
    background: #ffffff;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05);
  }
  
  .card-thumb {
    width: 64px;
    height: 64px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .card-info h4 {
    margin: 0 0 4px 0;
    font-size: 15px;
    font-weight: 700;
    color: #1f2937;
  }
  .card-info p {
    margin: 0;
    font-size: 13px;
    color: #6b7280;
    line-height: 1.4;
  }

  /* 업데이트 커밍순 티저 오버레이 (블러) */
  .store-overlay {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.6);
    backdrop-filter: blur(8px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 10;
    text-align: center;
    border-bottom-right-radius: 12px;
  }
  
  .store-overlay .coming-soon-text {
    margin: 16px 0 8px 0;
    font-size: 24px;
    font-weight: 800;
    color: #1f2937;
  }
  
  .store-overlay p {
    font-size: 15px;
    color: #4b5563;
  }

  .dockbar-wrapper {
    position: absolute;
    top: 10px;
    z-index: 1000;
  }
  
  .dock-top {
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
  }
  .dock-left {
    left: 16px;
    top: 50%;
    transform: translateY(-50%);
  }
  .dock-right {
    right: 16px;
    left: auto;
    top: 50%;
    transform: translateY(-50%);
  }

  .dockbar {
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(229, 231, 235, 0.8);
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
    border-radius: 100px;
    padding: 6px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    pointer-events: auto; /* Toolbar is always clickable */
  }
  .dockbar.vertical {
    flex-direction: column;
    border-radius: 20px;
    padding: 12px 10px;
  }
  
  .tool-group {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 50px;
  }
  .dockbar.vertical .tool-group {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 6px;
    border-radius: 12px;
  }

  .divider {
    width: 1px;
    height: 32px;
    background: #e5e7eb;
    margin: 0 4px;
  }
  .dockbar.vertical .divider {
    width: 32px;
    height: 1px;
    margin: 4px 0;
  }
  
  /* 그룹별 컬러 테마 (기획서 기반) */
  .blue-group .tool-btn:hover { background: #eff6ff; color: #2563eb; }
  .blue-group .tool-btn.active { background: #dbeafe; color: #1d4ed8; }
  
  .orange-group .tool-btn:hover { background: #fff7ed; color: #ea580c; }
  
  .purple-group .tool-btn:hover { background: #faf5ff; color: #9333ea; }
  
  .green-group .tool-btn:hover { background: #f0fdf4; color: #16a34a; }
  .green-group .tool-btn.active { background: #dcfce7; color: #15803d; }

  .tool-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 56px;
    height: 60px;
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
    color: #6b7280;
  }

  .tool-btn span {
    font-size: 11px;
    font-weight: 600;
  }

  /* 설정 패널 */
  .settings-panel {
    z-index: 99999;
    pointer-events: auto; /* 패널 클릭 허용 및 이벤트 흡수 */
    background: white;
    border-radius: 12px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.15);
    padding: 16px;
    width: max-content;
    max-width: 300px;
    border: 1px solid #f3f4f6;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    border-bottom: 1px solid #f3f4f6;
    padding-bottom: 8px;
  }

  .panel-title {
    font-weight: bold;
    color: #1f2937;
    text-transform: capitalize;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 20px;
    cursor: pointer;
    color: #9ca3af;
  }

  .setting-row {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
  }

  .setting-row .label {
    width: 50px;
    font-size: 13px;
    color: #4b5563;
    font-weight: 500;
  }

  .size-options {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .divider-vertical {
    width: 1px;
    height: 24px;
    background: #e5e7eb;
    margin: 0 4px;
  }

  .size-btn {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    border: 1px solid #e5e7eb;
    background: white;
    font-size: 12px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #4b5563;
  }
  .size-btn.active {
    background: #f3f4f6;
    border-color: #d1d5db;
    font-weight: bold;
  }

  /* 슬라이더(Range) 커스텀 */
  .slider {
    flex: 1;
    -webkit-appearance: none;
    height: 4px;
    background: #e5e7eb;
    border-radius: 2px;
    outline: none;
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #3b82f6;
    cursor: pointer;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
  }
  
  /* 앱 종료 버튼 */
  .app-exit-btn {
    position: absolute;
    top: 24px;
    right: 24px;
    z-index: 9999999;
    width: 44px;
    height: 44px;
    border-radius: 10px;
    border: 1px solid rgba(229, 231, 235, 0.8);
    background: rgba(255, 255, 255, 0.95);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.1);
    color: #ef4444;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    opacity: 0.9;
    pointer-events: auto;
    transition: all 0.2s;
  }
  .app-exit-btn:hover {
    opacity: 1.0;
    background: #fef2f2; /* Light red */
    border-color: #fca5a5;
    transform: scale(1.05);
  }

  .lightning-video-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    object-fit: cover;
    z-index: 2147483647; /* 가장 최상단 렌더링 */
    pointer-events: none; /* 영상이 재생되는 동안 클릭이 씹히지 않게 투과처리 */
    background: transparent;
  }

  .color-options {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .color-btn {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(0,0,0,0.1);
  }
  .color-btn.selected {
    box-shadow: 0 0 0 2px white, 0 0 0 4px #2563eb;
  }

  .color-picker-btn {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: none;
    background: conic-gradient(from 90deg, red, yellow, lime, aqua, blue, magenta, red);
    padding: 0;
    cursor: pointer;
    overflow: hidden;
    -webkit-appearance: none;
    box-shadow: inset 0 0 0 1px rgba(0,0,0,0.1);
  }
  .color-picker-btn::-webkit-color-swatch-wrapper {
    padding: 0;
  }
  .color-picker-btn::-webkit-color-swatch {
    border: none;
    border-radius: 50%;
    opacity: 0; /* 배경 무지개색이 보이도록 투명화 */
  }

  .slider {
    flex: 1;
  }
</style>
