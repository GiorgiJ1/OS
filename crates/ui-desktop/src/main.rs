#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use aios_assistant::Assistant;
use aios_memory::Database;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tracing::info;

struct AppState {
    db_path:         String,
    conversation_id: Mutex<uuid::Uuid>,
}

#[tauri::command]
async fn send_message(
    message: String,
    state:   State<'_, Arc<AppState>>,
    app:     AppHandle,
) -> Result<(), String> {
    let conv_id  = *state.conversation_id.lock().unwrap();
    let db_path  = state.db_path.clone();
    let app2     = app.clone();

    app.emit("aios-state", "thinking").ok();

    // Spawn a dedicated thread — rusqlite is not Send
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

            let (tx, mut rx) = mpsc::channel::<String>(64);

            // Forward tokens to frontend
            let app3 = app2.clone();
            tokio::spawn(async move {
                while let Some(token) = rx.recv().await {
                    app3.emit("aios-response-chunk", token).ok();
                }
            });

            match assistant.chat_stream_with_context(conv_id, &message, tx).await {
                Ok(_)  => {}
                Err(e) => {
                    app2.emit("aios-response-chunk", format!("Error: {}", e)).ok();
                }
            }

            app2.emit("aios-response-done", ()).ok();
            app2.emit("aios-state", "idle").ok();
        });
    });

    Ok(())
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

    // Get or create a conversation id for this session
    let conv_id = {
        let db = Database::open(&db_path).expect("db open");
        let assistant = Assistant::with_defaults(db).expect("assistant");
        let conv = assistant
            .new_conversation(Some("desktop session"))
            .expect("conv");
        info!("Desktop session: {}", conv.id);
        conv.id
    };

    let state = Arc::new(AppState {
        db_path,
        conversation_id: Mutex::new(conv_id),
    });

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![send_message])
        .run(tauri::generate_context!())
        .expect("tauri error");
}