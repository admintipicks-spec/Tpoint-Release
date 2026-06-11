use base64::{engine::general_purpose, Engine as _};
use screenshots::image::codecs::png::PngEncoder;
use screenshots::image::ColorType;
use screenshots::image::ImageEncoder;
use screenshots::Screen; // 추가된 부분

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;
// ... (아래는 동일)

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect},
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn capture_screen(x: i32, y: i32) -> Result<String, String> {
    let screen = Screen::from_point(x, y).map_err(|e| e.to_string())?;
    {
        let image_buffer = screen.capture().map_err(|e| e.to_string())?;

        let mut bytes: Vec<u8> = Vec::new();
        let encoder = PngEncoder::new(&mut bytes);

        encoder
            .write_image(
                &image_buffer,
                image_buffer.width(),
                image_buffer.height(),
                ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;

        let b64 = general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:image/png;base64,{}", b64))
    }
}

#[tauri::command]
async fn capture_screen_region(x: i32, y: i32, width: u32, height: u32) -> Result<String, String> {
    // 좌표(x, y)가 속한 모니터를 찾습니다 (멀티 모니터 지원)
    let screen = Screen::from_point(x, y).map_err(|e| e.to_string())?;

    // 메모리상에서 하드디스크 I/O 없이 지정된 영역만 Crop
    // (주의: 모니터별 로컬 좌표계 보정이 필요할 수 있으나, 전체화면 윈도우라 가정)
    let local_x = x - screen.display_info.x;
    let local_y = y - screen.display_info.y;
    let image_buffer = screen
        .capture_area(local_x, local_y, width, height)
        .map_err(|e| e.to_string())?;

    let mut bytes: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);

    encoder
        .write_image(
            &image_buffer,
            image_buffer.width(),
            image_buffer.height(),
            ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;

    let b64 = general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{}", b64))
}

// 글로벌 포인터 상태
static IS_POINTER_MODE: AtomicBool = AtomicBool::new(true);

// 툴바 위치 상태 (0=Top, 1=Bottom, 2=Left, 3=Right)
static TOOLBAR_POSITION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[tauri::command]
fn set_pointer_mode(is_pointer: bool) {
    IS_POINTER_MODE.store(is_pointer, Ordering::Relaxed);
}

#[tauri::command]
fn set_toolbar_position(position: u8) {
    TOOLBAR_POSITION.store(position, Ordering::Relaxed);
}

#[tauri::command]
fn force_exit() {
    std::process::exit(0);
}

#[tauri::command]
fn get_machine_uid() -> Result<String, String> {
    machine_uid::get().map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct VerifyRequest {
    #[serde(rename = "serialKey")]
    serial_key: String,
    #[serde(rename = "machineUid")]
    machine_uid: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct VerifyResponse {
    success: bool,
    message: String,
}

#[tauri::command]
async fn verify_license(serial_input: String) -> Result<VerifyResponse, String> {
    let machine_uid = machine_uid::get().unwrap_or_else(|_| "unknown_device".to_string());
    
    let req_data = VerifyRequest {
        serial_key: serial_input,
        machine_uid,
    };

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10)) // 구글 302 리다이렉션 완벽 추적
        .build()
        .map_err(|e| format!("Client build failed: {}", e))?;

    let res = client.post("https://script.google.com/macros/s/AKfycbyJXu_Tkc6r5WDTO8tXBfYhCSTkvUg54jPjCZJnl8OzDMezhp47JLI-rNtDJMitIw4m/exec")
        .json(&req_data)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let resp_data: VerifyResponse = res.json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    Ok(resp_data)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            #[cfg(windows)]
            {
                let app_handle = app.handle().clone();

                // 백그라운드 마우스 추적 스레드 (30ms 간격 폴링 = 약 33 FPS)
                std::thread::spawn(move || {
                    let mut was_ignored = false;

                    loop {
                        std::thread::sleep(Duration::from_millis(30));

                        let is_pointer = IS_POINTER_MODE.load(Ordering::Relaxed);

                        if let Some(window) = app_handle.get_webview_window("main") {
                            // 1. 펜/도형 모드: 캔버스가 클릭을 무조건 받아야 하므로 구멍을 막음
                            if !is_pointer {
                                if was_ignored {
                                    let _ = window.set_ignore_cursor_events(false);
                                    was_ignored = false;
                                }
                                continue;
                            }

                            // 2. 포인터 모드: 마우스 좌표를 실시간으로 판별
                            if let Ok(hwnd_ptr) = window.hwnd() {
                                let hwnd = HWND(hwnd_ptr.0 as _);
                                unsafe {
                                    let mut pt = POINT { x: 0, y: 0 };
                                    let mut rect = RECT {
                                        left: 0,
                                        top: 0,
                                        right: 0,
                                        bottom: 0,
                                    };

                                    // 글로벌 마우스 좌표와 앱 창의 좌표를 가져옴
                                    if GetCursorPos(&mut pt).is_ok()
                                        && GetWindowRect(hwnd, &mut rect).is_ok()
                                    {
                                        // 툴바 위치에 따른 히트박스 판별
                                        let pos = TOOLBAR_POSITION.load(Ordering::Relaxed);
                                        let is_over_toolbar = match pos {
                                            0 => pt.y >= rect.top && pt.y <= (rect.top + 100), // Top
                                            1 => pt.y >= (rect.bottom - 100) && pt.y <= rect.bottom, // Bottom
                                            2 => pt.x >= rect.left && pt.x <= (rect.left + 100), // Left
                                            3 => pt.x >= (rect.right - 100) && pt.x <= rect.right, // Right
                                            _ => false,
                                        };

                                        // 우상단 앱 종료 버튼 예외 (80x80 고정 영역 - UI 마진 및 보정값 포함)
                                        let is_over_exit = pt.x >= (rect.right - 80)
                                            && pt.x <= rect.right
                                            && pt.y >= rect.top
                                            && pt.y <= (rect.top + 80);

                                        // 둘 중 하나라도 만족하면 클릭 허용 (ignore = false)
                                        let should_ignore = !(is_over_toolbar || is_over_exit);

                                        // 상태가 변했을 때만 Tauri API 호출 (부하 방지)
                                        if should_ignore != was_ignored {
                                            let _ = window.set_ignore_cursor_events(should_ignore);
                                            was_ignored = should_ignore;
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
            Ok(())
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            capture_screen,
            capture_screen_region,
            set_pointer_mode,
            set_toolbar_position,
            force_exit,
            get_machine_uid,
            verify_license
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
