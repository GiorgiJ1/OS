use anyhow::Result;
use aios_system::{FilesystemWatcher, ActivityTracker, PatternEngine};
use aios_assistant::Assistant;
use aios_document::Ingestor;
use aios_embeddings::EmbeddingEngine;
use aios_memory::Database;
use std::io::{self, BufRead, Write};
use tokio::sync::mpsc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string())
                .as_str(),
        )
        .init();

    let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "C:\\Users\\giogu".to_string());
        format!("{}/.local/share/aios", home)
    });
    std::fs::create_dir_all(&data_dir)?;
    let db_path = format!("{}/aios.db", data_dir);

    info!("Opening database at {}", db_path);
    let db = Database::open(&db_path)?;
    let assistant = Assistant::with_defaults(db)?;
    let embedder = EmbeddingEngine::with_defaults()?;

    if !assistant.is_ready().await {
        eprintln!("Ollama is not running. Start it with: ollama serve");
        std::process::exit(1);
    }

    let conv = assistant.new_conversation(Some("AIOS session"))?;
    println!("AIOS ready. Conversation: {}", conv.id);
    println!("Commands:");
    println!("  /index <path>       — index a file");
    println!("  /index-dir <path>   — index a directory");
    println!("  /embed <doc-uuid>   — embed a document's chunks");
    println!("  /quit               — exit\n");
            // Start filesystem watcher
    // Start filesystem watcher
    let watcher = FilesystemWatcher::with_default_drives();
    let mut file_rx = watcher.start()?;

    // Spawn background thread for file watching + indexing
    let db_path_bg = db_path.clone();
    std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("background runtime");

    rt.block_on(async move {
        let db = Database::open(&db_path_bg).expect("db open");
        let embedder = EmbeddingEngine::with_defaults().expect("embedder");

        // SearchEngine for the background thread
        let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            format!("{}/.local/share/aios", home)
        });
        let index_path = format!("{}/tantivy", data_dir);
        let search = aios_search::SearchEngine::new(&index_path).expect("search engine");

        let mut pattern_tick = tokio::time::interval(
            tokio::time::Duration::from_secs(300)
        );

        loop {
            tokio::select! {
                Some(event) = file_rx.recv() => {
                    match &event {
                        aios_system::watcher::FileEvent::Created(path)
                        | aios_system::watcher::FileEvent::Modified(path) => {
                            {
                                let activity = ActivityTracker::new(&db);
                                activity.log_file_event(path, "modified").ok();
                            }
                            let ingestor = aios_document::Ingestor::new(&db);
                            if let Ok(doc) = ingestor.ingest_file(path) {
                                if let Ok(_) = embedder.embed_document_chunks(&db, doc.id).await {
                                    // Also index into Tantivy
                                    if let Ok(chunks) = db.get_chunks_for_document(doc.id) {
                                        for chunk in &chunks {
                                            search.index_chunk(chunk.id, &chunk.content).ok();
                                        }
                                    }
                                    tracing::info!(
                                        "Auto-indexed + search updated: {:?}",
                                        doc.title
                                    );
                                }
                            }
                        }
                        aios_system::watcher::FileEvent::Deleted(path) => {
                            let activity = ActivityTracker::new(&db);
                            activity.log_file_event(path, "deleted").ok();
                        }
                    }
                }
                _ = pattern_tick.tick() => {
                    let patterns = PatternEngine::new(&db);
                    patterns.analyse_and_store().ok();
                }
            }
        }
    });
});

println!("Filesystem watcher active on D:\\ and G:\\");

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

        if input == "/quit" || input == "/exit" {
            break;
        }

        if let Some(path) = input.strip_prefix("/index ") {
            let path = std::path::Path::new(path.trim());
            let ingestor = Ingestor::new(assistant.db());
            match ingestor.ingest_file(path) {
                Ok(doc) => {
                    println!("Indexed: {:?} ({})", doc.title, doc.id);
                    // Auto-embed and auto-index into Tantivy
                    println!("Embedding chunks...");
                    match embedder.embed_document_chunks(assistant.db(), doc.id).await {
                        Ok(n) => {
                            println!("Embedded {} chunks", n);
                            // Index chunks into Tantivy
                            let chunks = assistant.db().get_chunks_for_document(doc.id)?;
                            for chunk in &chunks {
                                assistant.search_engine().index_chunk(chunk.id, &chunk.content)?;
                            }
                            println!("Search index updated. Document ready to query.");
                        }
                        Err(e) => println!("Embedding error: {}", e),
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        if let Some(dir) = input.strip_prefix("/index-dir ") {
            let path = std::path::Path::new(dir.trim());
            let ingestor = Ingestor::new(assistant.db());
            match ingestor.ingest_directory(path) {
                Ok(docs) => {
                    println!("Indexed {} files", docs.len());
                    for doc in &docs {
                        match embedder.embed_document_chunks(assistant.db(), doc.id).await {
                            Ok(n) => {
                                let chunks = assistant.db().get_chunks_for_document(doc.id)?;
                                for chunk in &chunks {
                                    assistant.search_engine().index_chunk(chunk.id, &chunk.content)?;
                                }
                                println!("  {:?} — {} chunks embedded", doc.title, n);
                            }
                            Err(e) => println!("  {:?} — embedding error: {}", doc.title, e),
                        }
                    }
                    println!("All documents ready to query.");
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        if let Some(id_str) = input.strip_prefix("/embed ") {
            match uuid::Uuid::parse_str(id_str.trim()) {
                Ok(doc_id) => {
                    match embedder.embed_document_chunks(assistant.db(), doc_id).await {
                        Ok(n) => println!("Embedded {} chunks", n),
                        Err(e) => println!("Error: {}", e),
                    }
                }
                Err(_) => println!("Usage: /embed <document-uuid>"),
            }
            continue;
        }

        if input == "/memories" {
            match assistant.db().list_memories() {
                Ok(mems) if mems.is_empty() => println!("No memories stored yet."),
                Ok(mems) => {
                    println!("Stored memories:");
                    for (key, value, source) in &mems {
                        println!(
                            "  {} = {} ({})",
                            key,
                            value,
                            source.as_deref().unwrap_or("unknown")
                        );
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        if let Some(rest) = input.strip_prefix("/remember ") {
            if let Some((key, value)) = rest.split_once('=') {
                match assistant.db().set_memory(key.trim(), value.trim(), Some("user")) {
                    Ok(_) => println!("Remembered: {} = {}", key.trim(), value.trim()),
                    Err(e) => println!("Error: {}", e),
                }
            } else {
                println!("Usage: /remember key = value");
            }
            continue;
        }

        if let Some(key) = input.strip_prefix("/forget ") {
            match assistant.db().delete_memory(key.trim()) {
                Ok(_) => println!("Forgotten: {}", key.trim()),
                Err(e) => println!("Error: {}", e),
            }
            continue;
        }

        // Normal chat with document context
        let (tx, mut rx) = mpsc::channel::<String>(64);

        print!("AIOS: ");
        io::stdout().flush()?;

        tokio::select! {
            result = assistant.chat_stream_with_context(conv.id, &input, tx) => {
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