#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use aios_assistant::Assistant;
use aios_memory::Database;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

struct AppState {
    db_path:         String,
    conversation_id: Mutex<Uuid>,
}

#[tauri::command]
async fn send_message(
    message: String,
    state:   State<'_, Arc<AppState>>,
    app:     AppHandle,
) -> Result<(), String> {
    let conv_id = *state.conversation_id.lock().unwrap();
    let db_path = state.db_path.clone();
    let app2    = app.clone();

    app.emit("aios-state", "thinking").ok();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");

        rt.block_on(async move {
            let db = match Database::open(&db_path) {
                Ok(d)  => d,
                Err(e) => {
                    app2.emit("aios-response-chunk", format!("Error: {}", e)).ok();
                    app2.emit("aios-response-done", ()).ok();
                    app2.emit("aios-state", "idle").ok();
                    return;
                }
            };
            let assistant = match Assistant::with_defaults(db) {
                Ok(a)  => a,
                Err(e) => {
                    app2.emit("aios-response-chunk", format!("Error: {}", e)).ok();
                    app2.emit("aios-response-done", ()).ok();
                    app2.emit("aios-state", "idle").ok();
                    return;
                }
            };

            let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
            let app3 = app2.clone();
            tokio::spawn(async move {
                while let Some(token) = rx.recv().await {
                    app3.emit("aios-response-chunk", token).ok();
                }
            });

            match assistant.chat_stream_with_context(conv_id, &message, tx).await {
                Ok(_)  => {}
                Err(e) => { app2.emit("aios-response-chunk", format!("Error: {}", e)).ok(); }
            }

            app2.emit("aios-response-done", ()).ok();
            app2.emit("aios-state", "idle").ok();
        });
    });

    Ok(())
}

#[tauri::command]
async fn toggle_chat(duck_x: i32, duck_y: i32, app: AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("chat") {
        if chat.is_visible().unwrap_or(false) {
            chat.hide().map_err(|e| e.to_string())?;
        } else {
            // Position chat above the duck
            let chat_y = (duck_y - 510).max(0);
            let chat_x = duck_x.max(0).min(1520);
            chat.set_position(tauri::PhysicalPosition::new(chat_x, chat_y))
                .map_err(|e| e.to_string())?;
            chat.show().map_err(|e| e.to_string())?;
            chat.set_focus().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn move_window(x: i32, y: i32, app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("duck") {
        window.set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_screen_size(app: AppHandle) -> Result<(u32, u32), String> {
    if let Some(window) = app.get_webview_window("duck") {
        let monitor = window.current_monitor()
            .map_err(|e| e.to_string())?
            .ok_or("no monitor")?;
        let size = monitor.size();
        return Ok((size.width, size.height));
    }
    Ok((1920, 1080))
}

fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "C:\\Users\\user".to_string());
        format!("{}/.local/share/aios", home)
    });
    std::fs::create_dir_all(&data_dir).ok();
    let db_path = format!("{}/aios.db", data_dir);

    let conv_id = {
        let db        = Database::open(&db_path).expect("db open");
        let assistant = Assistant::with_defaults(db).expect("assistant");
        assistant.new_conversation(Some("desktop session"))
            .expect("conv").id
    };

    let state = Arc::new(AppState {
        db_path,
        conversation_id: Mutex::new(conv_id),
    });

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            send_message,
            toggle_chat,
            move_window,
            get_screen_size,
        ])
        .run(tauri::generate_context!())
        .expect("tauri error");
}