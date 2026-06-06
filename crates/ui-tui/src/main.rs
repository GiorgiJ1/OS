use anyhow::Result;
use aios_assistant::Assistant;
use aios_memory::Database;
use std::io::{self, BufRead, Write};
use tokio::sync::mpsc;
use tracing::info;
use aios_embeddings::EmbeddingEngine;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string())
                .as_str(),
        )
        .init();

    let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{}/.local/share/aios", home)
    });
    std::fs::create_dir_all(&data_dir)?;
    let db_path = format!("{}/aios.db", data_dir);

    info!("Opening database at {}", db_path);
    let db = Database::open(&db_path)?;
    let assistant = Assistant::with_defaults(db)?;
    let ingestor = aios_document::Ingestor::new(assistant.db());
    let embedder = EmbeddingEngine::with_defaults()?;

    if !assistant.is_ready().await {
        eprintln!("Ollama is not running. Start it with: ollama serve");
        std::process::exit(1);
    }

    let conv = assistant.new_conversation(Some("AIOS session"))?;
    println!("AIOS ready. Conversation: {}", conv.id);
    println!("Type your message and press Enter. /quit to exit.\n");

    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            continue;
        }

        // Built-in commands
        if input == "/quit" || input == "/exit" {
            break;
        }

        if let Some(path) = input.strip_prefix("/index ") {
            let path = std::path::Path::new(path.trim());
            match ingestor.ingest_file(path) {
                Ok(doc) => println!("Indexed: {:?} ({})", doc.title, doc.id),
                Err(e)  => println!("Error: {}", e),
            }
            continue;
        }

        if let Some(dir) = input.strip_prefix("/index-dir ") {
            let path = std::path::Path::new(dir.trim());
            match ingestor.ingest_directory(path) {
                Ok(docs) => println!("Indexed {} files", docs.len()),
                Err(e)   => println!("Error: {}", e),
            }
            continue;
        }

        if let Some(id_str) = input.strip_prefix("/embed ") {
            match uuid::Uuid::parse_str(id_str.trim()) {
                Ok(doc_id) => {
                    match embedder.embed_document_chunks(assistant.db(), doc_id).await {
                        Ok(n)  => println!("Embedded {} chunks", n),
                        Err(e) => println!("Error: {}", e),
                    }
                }
                Err(_) => println!("Usage: /embed <document-uuid>"),
        }
            continue;
        }

        // Normal chat
        let (tx, mut rx) = mpsc::channel::<String>(64);

        print!("AIOS: ");
        io::stdout().flush()?;

        tokio::select! {
            result = assistant.chat_stream(conv.id, &input, tx) => {
                result?;
            }
            _ = async {
                while let Some(token) = rx.recv().await {
                    print!("{}", token);
                    io::stdout().flush().ok();
                }
            } => {}
        }

        while let Ok(token) = rx.try_recv() {
            print!("{}", token);
            io::stdout().flush().ok();
        }

        println!("\n");
    }

    Ok(())
}