<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Konva from 'konva';
  import { invoke } from '@tauri-apps/api/core';
  import { toolManager } from './ToolManager.svelte.ts';
  import { audioEngine } from './AudioEngine.ts';
  import confetti from 'canvas-confetti';

  let container: HTMLDivElement;
  let stage: Konva.Stage;
  let drawingLayer: Konva.Layer;
  
  let isDrawing = false;
  let currentLine: Konva.Line | Konva.Shape | null = null;
  let currentPoints: number[] = [];
  let startPos: {x: number, y: number} | null = null;
  let strokeCount = 0;

  let magnifierGroup: Konva.Group | null = null;
  let magnifierBorder: Konva.Circle | null = null;
  let screenshotKonvaImage: Konva.Image | null = null;
  let magnifierImgObj: HTMLImageElement | null = null;
  let magnifierScale = 2.0;
  let magnifierRadius = 150;

  // 캡처 위젯 상태 (메모리 관리용)
  let captureKonvaWidgets: { id: string, image: Konva.Image, tr: Konva.Transformer }[] = [];
  let captureSelectionRect: Konva.Rect | null = null;

  // 텍스트 베이킹 상태
  let textInput = $state({
    active: false,
    x: 0,
    y: 0,
    value: ''
  });

  let lastTriggerId = 0;

  $effect(() => {
    try {
      const tool = toolManager.activeTool;
      
      // 텍스트 도구에서 다른 도구로 변경 시 자동 굽기(Baking)
      if (tool !== 'text' && textInput.active) {
        bakeText();
      }

      if (tool === 'magnifier') {
        enableMagnifier();
      } else {
        disableMagnifier();
      }

      const trigger = toolManager.effectTrigger;
      if (trigger && trigger.id !== lastTriggerId) {
        lastTriggerId = trigger.id;
        handleEffectTrigger(trigger.name);
      }
    } catch (err) {
      console.error("[CanvasLayer] $effect 에러 (연쇄 붕괴 방어):", err);
    }
  });

  // 애니메이션 추적용
  let activeEffects: { [key: string]: { anim?: Konva.Animation, interval?: any, nodes: any[] } } = {};
  function clearEffect(type: string) {
    const effect = activeEffects[type];
    if (effect) {
      if (effect.anim) effect.anim.stop();
      if (effect.interval) clearInterval(effect.interval);
      effect.nodes.forEach(n => n.destroy());
      delete activeEffects[type];
    }
  }

  function handleEffectTrigger(effectName: string) {
    if (effectName === 'clear_all') {
      // 위젯 완벽 삭제 (메모리 누수 차단)
      for (const cw of captureKonvaWidgets) {
        cw.image.destroy();
        cw.tr.destroy();
        const imgObj = cw.image.image() as HTMLImageElement;
        if (imgObj) imgObj.src = '';
      }
      captureKonvaWidgets = [];
      toolManager.clearCaptureWidgets();
      
      drawingLayer.destroyChildren();
      strokeCount = 0;
      drawingLayer.clearCache();
      drawingLayer.batchDraw();
    } else if (effectName === 'fireworks') {
      clearEffect('fireworks'); // 중복 실행 방지
      audioEngine.playFireworks();
      const duration = 3 * 1000;
      const animationEnd = Date.now() + duration;
      const defaults = { startVelocity: 30, spread: 360, ticks: 60, zIndex: 10000 };
      const interval: any = setInterval(function() {
        const timeLeft = animationEnd - Date.now();
        if (timeLeft <= 0) {
          clearEffect('fireworks');
          return;
        }
        const particleCount = 50 * (timeLeft / duration);
        confetti(Object.assign({}, defaults, { particleCount, origin: { x: Math.random(), y: Math.random() - 0.2 } }));
      }, 250);
      activeEffects['fireworks'] = { interval, nodes: [] };
    } else if (effectName === 'lightning') {
      // 번개 이펙트의 비주얼(video)은 +page.svelte에서 처리하므로,
      // 여기서는 오디오 재생 트리거만 남겨둡니다. (또는 +page.svelte에서 오디오도 처리 가능)
      // 요구사항에 따라 캔버스 하얗게 번쩍이던 Konva 효과 영구 폐기.
    } else if (effectName === 'rainbow') {
      clearEffect('rainbow');
      audioEngine.playRainbow();
      const colors = ['#FF0000', '#FF7F00', '#FFFF00', '#00FF00', '#0000FF', '#4B0082', '#9400D3'];
      const arcs: Konva.Arc[] = [];
      const centerX = stage.width() / 2;
      const centerY = stage.height();
      
      const maxRadius = Math.max(stage.width(), stage.height());
      const bandWidth = maxRadius * 0.04;
      const baseRadius = maxRadius * 0.4;

      colors.forEach((color, i) => {
        const arc = new Konva.Arc({
          x: centerX, y: centerY, 
          innerRadius: baseRadius + i * bandWidth, 
          outerRadius: baseRadius + (i + 1) * bandWidth, 
          angle: 180, rotation: 180, fill: color, opacity: 0, listening: false
        });
        arcs.push(arc);
        drawingLayer.add(arc);
      });
      const anim = new Konva.Animation((frame) => {
        if (!frame) return;
        arcs.forEach(arc => {
          if (frame.time <= 500) arc.opacity((frame.time / 500) * 0.7);
          else if (frame.time > 2000) arc.opacity(Math.max(0, 0.7 - ((frame.time - 2000) / 1000) * 0.7));
          else arc.opacity(0.7);
        });
        if (frame.time > 3000) { clearEffect('rainbow'); }
      }, drawingLayer);
      anim.start();
      activeEffects['rainbow'] = { anim, nodes: arcs };
    }
  }

  async function enableMagnifier() {
    if (magnifierGroup) return; // 이미 활성화됨
    try {
      const base64Str = await invoke<string>('capture_screen', { x: Math.floor(window.screenX), y: Math.floor(window.screenY) });
      magnifierImgObj = new Image();
      magnifierImgObj.src = base64Str;
      magnifierImgObj.onload = () => {
        // 비동기 콜백 시점에 이미 다른 툴로 바뀌었으면 생성 중단 (Ghost Layer 방지)
        if (toolManager.activeTool !== 'magnifier') return;

        screenshotKonvaImage = new Konva.Image({
          image: magnifierImgObj!,
          listening: false,
        });

        magnifierGroup = new Konva.Group({
          listening: false,
          clipFunc: (ctx) => {
            const pos = stage?.getPointerPosition();
            if (pos) {
              ctx.arc(pos.x, pos.y, magnifierRadius, 0, Math.PI * 2, false);
            }
          }
        });
        
        magnifierBorder = new Konva.Circle({
          radius: magnifierRadius,
          stroke: '#2563eb',
          strokeWidth: 4,
          listening: false,
        });

        magnifierGroup.add(screenshotKonvaImage);
        drawingLayer.add(magnifierGroup);
        drawingLayer.add(magnifierBorder);
        updateMagnifierPosition();
      };
    } catch (e) {
      console.error("[CanvasLayer] 돋보기 캡처 실패:", e);
    }
  }

  function disableMagnifier() {
    try {
      if (magnifierGroup) {
        magnifierGroup.destroy();
        magnifierGroup = null;
      }
      if (magnifierBorder) {
        magnifierBorder.destroy();
        magnifierBorder = null;
      }
      screenshotKonvaImage = null;
      if (magnifierImgObj) {
        magnifierImgObj.src = '';
        magnifierImgObj = null;
      }
      if (drawingLayer) drawingLayer.batchDraw();
    } catch (e) {
      console.error("[CanvasLayer] disableMagnifier 에러:", e);
    }
  }

  function updateMagnifierPosition() {
    if (!magnifierGroup || !screenshotKonvaImage || !magnifierBorder || toolManager.activeTool !== 'magnifier') return;
    const pos = stage.getPointerPosition();
    if (!pos) return;

    screenshotKonvaImage.scale({ x: magnifierScale, y: magnifierScale });
    screenshotKonvaImage.x(pos.x * (1 - magnifierScale));
    screenshotKonvaImage.y(pos.y * (1 - magnifierScale));

    magnifierBorder.x(pos.x);
    magnifierBorder.y(pos.y);

    drawingLayer.batchDraw();
  }

  export function removeWidget(id: string) {
    const idx = captureKonvaWidgets.findIndex(w => w.id === id);
    if (idx > -1) {
      const cw = captureKonvaWidgets[idx];
      cw.image.destroy();
      cw.tr.destroy();
      const imgObj = cw.image.image() as HTMLImageElement;
      if (imgObj) imgObj.src = ''; // GC 메모리 반환
      captureKonvaWidgets.splice(idx, 1);
      drawingLayer.batchDraw();
      toolManager.removeCaptureWidget(id);
    }
  }

  export function toggleLockWidget(id: string) {
    const cw = captureKonvaWidgets.find(w => w.id === id);
    const state = toolManager.captureWidgets.find(w => w.id === id);
    if (cw && state) {
      const newLocked = !state.isLocked;
      cw.image.draggable(!newLocked);
      cw.tr.visible(!newLocked);
      drawingLayer.batchDraw();
      toolManager.updateCaptureWidget(id, state.x, state.y, state.w, state.h, newLocked);
    }
  }

  let cursorStyle = $derived(getCursorStyle(toolManager.activeTool));

  function getCursorStyle(tool: string) {
    switch (tool) {
      case 'pen':
      case 'highlighter':
        return 'crosshair';
      case 'circle':
      case 'box':
        return 'crosshair';
      case 'eraser':
        return 'cell';
      case 'magnifier':
        return 'zoom-in';
      case 'capture':
        return 'crosshair';
      case 'here':
      case 'ding':
      case 'clap':
        return `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64' viewBox='0 0 64 64'><text y='48' font-size='48'>👆</text></svg>") 32 10, pointer`;
      default:
        return 'default';
    }
  }

  onMount(() => {
    try {
      stage = new Konva.Stage({
        container,
        width: window.innerWidth,
        height: window.innerHeight,
      });

      console.log(`[CanvasLayer] Stage Mounted. size: ${window.innerWidth}x${window.innerHeight}, container:`, container);

      drawingLayer = new Konva.Layer();
      stage.add(drawingLayer);

      window.addEventListener('resize', handleResize);

      stage.on('mousedown touchstart', handlePointerDown);
      stage.on('mousemove touchmove', handlePointerMove);
      stage.on('mouseup touchend', handlePointerUp);
      stage.on('wheel', handleWheel);
      
      console.log("[CanvasLayer] Konva Stage 초기화 성공");
    } catch (e) {
      console.error("[CanvasLayer] 초기화 에러:", e);
    }

    return () => {
      window.removeEventListener('resize', handleResize);
      if (stage) stage.destroy();
    };
  });

  function handleResize() {
    stage.width(window.innerWidth);
    stage.height(window.innerHeight);
  }

  function handlePointerDown(e: any) {
    try {
      const tool = toolManager.activeTool;
      // 'here', 'ding', 'clap'은 캔버스 클릭 시 동작해야 하므로 예외 처리에서 제외합니다.
      if (['pointer', 'magnifier', 'lightning', 'fireworks', 'rainbow', 'clear_all', 'mute'].includes(tool)) return;
      
      isDrawing = true;
      const pos = stage.getPointerPosition();
      if (!pos) return;
      
      if (tool === 'text') {
        if (textInput.active) bakeText();
        
        textInput.x = pos.x;
        textInput.y = pos.y;
        textInput.value = '';
        textInput.active = true;
        toolManager.setTextInputActive(true);
        return;
      }

      if (tool === 'capture') {
        if (toolManager.captureWidgets.length >= 3) {
          toolManager.showToast('캡처는 최대 3개까지 고정할 수 있습니다. 더 추가하려면 기존 캡처를 삭제해 주세요.');
          isDrawing = false;
          return;
        }
        captureSelectionRect = new Konva.Rect({
          x: pos.x, y: pos.y, width: 0, height: 0,
          fill: 'rgba(37, 99, 235, 0.1)',
          stroke: '#2563eb',
          strokeWidth: 2,
          dash: [5, 5],
          listening: false
        });
        drawingLayer.add(captureSelectionRect);
        drawingLayer.batchDraw();
      }

      if (['here', 'ding', 'clap'].includes(tool)) {
        clearEffect(tool);
        
        if (tool === 'here') audioEngine.playHere();
        else if (tool === 'ding') audioEngine.playDing();
        else if (tool === 'clap') audioEngine.playClap();
        
        const strokeColor = tool === 'here' ? '#ff0000' : (tool === 'ding' ? '#ffff00' : '#00ff00');
        
        const ripple = new Konva.Circle({
          x: pos.x, y: pos.y, radius: 10, stroke: strokeColor, strokeWidth: 5, opacity: 1, listening: false
        });
        drawingLayer.add(ripple);
        
        const anim = new Konva.Animation((frame) => {
          if (!frame) return;
          ripple.radius(10 + frame.time * 0.2);
          ripple.opacity(Math.max(0, 1 - frame.time / 1000));
          if (frame.time > 1000) { clearEffect(tool); }
        }, drawingLayer);
        anim.start();
        activeEffects[tool] = { anim, nodes: [ripple] };
        return;
      }
      
      const config = toolManager.getConfig(tool);
      const color = config.color;
      console.log(`[CanvasLayer] Down at x: ${pos.x}, y: ${pos.y}, tool: ${tool}, color: ${color}`);
      startPos = { x: pos.x, y: pos.y };

      if (tool === 'eraser') {
        return; // 지우개는 드래그 시점에 처리
      }

      if (tool === 'pen' || tool === 'highlighter') {
        currentPoints = [pos.x, pos.y];
        const opacity = tool === 'highlighter' ? 0.4 : 1;
        const size = config.size;

        currentLine = new Konva.Line({
          stroke: color,
          strokeWidth: size * (tool === 'highlighter' ? 2.5 : 1),
          lineCap: 'round',
          lineJoin: 'round',
          tension: 0.5,
          opacity: tool === 'highlighter' ? (config.opacity ?? 0.4) : 1,
          globalCompositeOperation: 'source-over',
          points: currentPoints,
        });
        drawingLayer.add(currentLine);
      } else if (tool === 'circle') {
        currentLine = new Konva.Ellipse({
          x: pos.x,
          y: pos.y,
          radiusX: 0,
          radiusY: 0,
          stroke: color,
          strokeWidth: config.size,
        });
        drawingLayer.add(currentLine);
      } else if (tool === 'box') {
        currentLine = new Konva.Rect({
          x: pos.x,
          y: pos.y,
          width: 0,
          height: 0,
          stroke: color,
          strokeWidth: config.size,
        });
        drawingLayer.add(currentLine);
      }
      drawingLayer.batchDraw();
    } catch (err) {
      console.error("[CanvasLayer] handlePointerDown 에러:", err);
      isDrawing = false;
    }
  }

  function handlePointerMove(e: any) {
    try {
      const tool = toolManager.activeTool;
      if (tool === 'magnifier') {
        updateMagnifierPosition();
        return;
      }

      if (!isDrawing) return;
      const pos = stage.getPointerPosition();
      if (!pos || !startPos) return;

      if (tool === 'eraser') {
        const eraserRadius = toolManager.getConfig('eraser').size * 3;
        const shapes = drawingLayer.getChildren();
        
        // 지우개 로직 실행 전 캐시 해제 (교차 감지를 위해)
        drawingLayer.clearCache();
        
        let removed = false;
        for (const shape of shapes) {
          if (shape.className === 'Transformer' || shape.className === 'Image' || shape.className === 'Group') continue;
          
          const box = shape.getClientRect();
          if (
            pos.x >= box.x - eraserRadius &&
            pos.x <= box.x + box.width + eraserRadius &&
            pos.y >= box.y - eraserRadius &&
            pos.y <= box.y + box.height + eraserRadius
          ) {
            shape.destroy();
            removed = true;
          }
        }
        
        if (removed) {
          // 지운 후 렌더링 최적화를 위해 다시 캐싱 (획이 많을 때만)
          if (strokeCount > 100) {
            drawingLayer.cache({ x: 0, y: 0, width: stage.width(), height: stage.height() });
          }
          drawingLayer.batchDraw();
        }
        return;
      }

      if (tool === 'capture' && captureSelectionRect) {
        const targetX = Math.min(startPos.x, pos.x);
        const targetY = Math.min(startPos.y, pos.y);
        const w = Math.abs(pos.x - startPos.x);
        const h = Math.abs(pos.y - startPos.y);
        captureSelectionRect.x(targetX);
        captureSelectionRect.y(targetY);
        captureSelectionRect.width(w);
        captureSelectionRect.height(h);
        drawingLayer.batchDraw();
        return;
      }

      if (!currentLine) return;

      if (tool === 'pen' || tool === 'highlighter') {
        let currentX = pos.x;
        let currentY = pos.y;
        
        if (e.evt.shiftKey) {
          const dx = Math.abs(pos.x - startPos.x);
          const dy = Math.abs(pos.y - startPos.y);
          if (dx > dy) {
            currentY = startPos.y;
          } else {
            currentX = startPos.x;
          }
          currentPoints = [startPos.x, startPos.y, currentX, currentY];
        } else {
          currentPoints.push(currentX, currentY);
        }
        
        (currentLine as Konva.Line).points(currentPoints);
      } else if (tool === 'circle') {
        const ellipse = currentLine as Konva.Ellipse;
        let rx = Math.abs(pos.x - startPos.x);
        let ry = Math.abs(pos.y - startPos.y);
        
        if (e.evt.shiftKey) {
          const maxR = Math.max(rx, ry);
          rx = maxR;
          ry = maxR;
        }
        ellipse.radiusX(rx);
        ellipse.radiusY(ry);
      } else if (tool === 'box') {
        const rect = currentLine as Konva.Rect;
        let w = Math.abs(pos.x - startPos.x);
        let h = Math.abs(pos.y - startPos.y);
        
        if (e.evt.shiftKey) {
          const size = Math.max(w, h);
          w = size;
          h = size;
        }
        
        const targetX = pos.x < startPos.x ? startPos.x - w : startPos.x + w;
        const targetY = pos.y < startPos.y ? startPos.y - h : startPos.y + h;

        rect.x(Math.min(startPos.x, targetX));
        rect.y(Math.min(startPos.y, targetY));
        rect.width(w);
        rect.height(h);
      }
      drawingLayer.batchDraw();
    } catch (err) {
      console.error("[CanvasLayer] handlePointerMove 에러:", err);
    }
  }

  function handlePointerUp() {
    if (toolManager.activeTool === 'capture' && captureSelectionRect) {
      const x = captureSelectionRect.x();
      const y = captureSelectionRect.y();
      const w = captureSelectionRect.width();
      const h = captureSelectionRect.height();
      captureSelectionRect.destroy();
      captureSelectionRect = null;
      drawingLayer.batchDraw();
      
      // 너무 작은 영역 캡처 방지
      if (w > 10 && h > 10) {
        invoke<string>('capture_screen_region', { x: Math.floor(window.screenX + x), y: Math.floor(window.screenY + y), width: Math.floor(w), height: Math.floor(h) })
          .then(base64 => {
            createCaptureWidget(base64, x, y, w, h);
          })
          .catch(e => {
            console.error("[CanvasLayer] capture_screen_region failed:", e);
            toolManager.showToast('캡처를 실패했습니다.');
          });
      }
    }

    isDrawing = false;
    currentLine = null;
    currentPoints = [];
    startPos = null;

    // 그리기 최적화 (Caching)
    const tool = toolManager.activeTool;
    if (['pen', 'highlighter', 'circle', 'box'].includes(tool)) {
      strokeCount++;
      // 일정 획 이상 그려지면 레이어 전체를 하나의 비트맵으로 캐싱하여 렌더링 병목 차단
      if (strokeCount > 100) {
        drawingLayer.cache({
          x: 0, y: 0, width: stage.width(), height: stage.height(),
          pixelRatio: window.devicePixelRatio || 1
        });
      }
    }
  }

  function createCaptureWidget(base64: string, x: number, y: number, w: number, h: number) {
    const id = 'widget_' + Date.now();
    const imgObj = new Image();
    imgObj.src = base64;
    imgObj.onload = () => {
      const konvaImage = new Konva.Image({
        image: imgObj,
        x, y, width: w, height: h,
        draggable: true,
      });

      const tr = new Konva.Transformer({
        nodes: [konvaImage],
        keepRatio: true,
        boundBoxFunc: (oldBox, newBox) => {
          if (newBox.width < 50 || newBox.height < 50) return oldBox;
          return newBox;
        },
      });

      const updateState = () => {
        const box = konvaImage.getClientRect();
        toolManager.updateCaptureWidget(id, box.x, box.y, box.width, box.height);
      };

      konvaImage.on('dragmove', updateState);
      konvaImage.on('transform', updateState);

      drawingLayer.add(konvaImage);
      drawingLayer.add(tr);
      drawingLayer.batchDraw();

      captureKonvaWidgets.push({ id, image: konvaImage, tr });
      toolManager.updateCaptureWidget(id, x, y, w, h, false);
    };
  }

  function handleWheel(e: any) {
    if (toolManager.activeTool !== 'magnifier') return;
    e.evt.preventDefault();
    
    if (e.evt.shiftKey) {
      const delta = e.evt.deltaY < 0 ? 10 : -10;
      magnifierRadius = Math.max(50, Math.min(400, magnifierRadius + delta));
      if (magnifierBorder) {
        magnifierBorder.radius(magnifierRadius);
      }
    } else {
      const delta = e.evt.deltaY < 0 ? 0.2 : -0.2;
      magnifierScale = Math.max(1.5, Math.min(4.0, magnifierScale + delta));
    }
    
    // requestAnimationFrame을 사용하여 휠 스크롤 렌더링 최적화
    requestAnimationFrame(updateMagnifierPosition);
  }

  function bakeText() {
    if (!textInput.active || textInput.value.trim() === '') {
      textInput.active = false;
      toolManager.setTextInputActive(false);
      return;
    }
    
    // 캐시 일시 해제 후 텍스트 추가
    drawingLayer.clearCache();

    const config = toolManager.getConfig('text');
    const textNode = new Konva.Text({
      x: textInput.x,
      y: textInput.y,
      text: textInput.value,
      fontSize: config.size * 2,
      fill: config.color,
      fontFamily: 'sans-serif',
      fontStyle: config.bold ? 'bold' : 'normal',
    });
    drawingLayer.add(textNode);
    strokeCount++;
    
    if (strokeCount > 100) {
      drawingLayer.cache({ x: 0, y: 0, width: stage.width(), height: stage.height() });
    }
    drawingLayer.batchDraw();
    
    textInput.active = false;
    toolManager.setTextInputActive(false);
  }
</script>

<div bind:this={container} class="canvas-container" style="cursor: {cursorStyle}"></div>

{#if textInput.active}
  <!-- svelte-ignore a11y_autofocus -->
  <textarea
    class="text-baker"
    style="left: {textInput.x}px; top: {textInput.y}px; font-size: {toolManager.getConfig('text').size * 2}px; color: {toolManager.getConfig('text').color}; font-weight: {toolManager.getConfig('text').bold ? 'bold' : 'normal'};"
    bind:value={textInput.value}
    onblur={bakeText}
    onkeydown={(e) => {
      // Shift+Enter나 그냥 Enter로 완료 처리(취향따라 조절 가능)
      if (e.key === 'Enter' && e.shiftKey) {
        e.preventDefault();
        bakeText();
      }
    }}
    autofocus
  ></textarea>
{/if}

<style>
  .canvas-container {
    width: 100vw;
    height: 100vh;
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: inherit; /* 부모(page)의 이벤트를 따름 */
  }

  .text-baker {
    position: absolute;
    z-index: 10000;
    pointer-events: auto;
    background: transparent;
    border: 2px dashed #2563eb;
    outline: none;
    resize: both;
    font-family: 'sans-serif';
    min-width: 100px;
    min-height: 40px;
    padding: 4px;
    /* 입력 시에는 약간 불투명한 흰색 배경을 줘서 글씨가 잘 보이게 할 수도 있음 */
    background-color: rgba(255, 255, 255, 0.5);
  }
</style>
