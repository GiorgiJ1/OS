```bash
cat > README.md << 'EOF'
# AIOS — AI-Native Operating Layer for Linux

> *What if your operating system understood you?*

AIOS replaces the traditional desktop workflow with a single conversational interface. Instead of hunting through applications and folders, you talk to AIOS — and it finds, summarizes, and acts on information across your entire system.

```
Traditional:  You → Desktop → Applications → Files → Information
AIOS:         You → Assistant → Information / Actions


Built from scratch in Rust. Runs fully offline. Your data never leaves your machine.

---

## Status

This project is in active early development. The table below reflects the current state.

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Core assistant loop + Ollama streaming | ✅ Done |
| 2 | Document ingestion (PDF, DOCX, TXT, MD) | ✅ Done |
| 3 | Embedding engine (nomic-embed-text) | ✅ Done |
| 4 | Hybrid search (Tantivy + cosine similarity) | ✅ Done |
| 5 | Memory system (cross-session learning) | ✅ Done |
| 6 | Filesystem watcher + always-on daemon | ✅ Done|
| 7 | Pattern learning + proactive insights | ✅ Done|
| 8 | Tauri overlay (system tray + global hotkey) | 🔄 Next |
| 9 | Voice interface (whisper.cpp) | ⬜ Planned |
| 10 | Linux system integration + distribution | ⬜ Planned |

---

## 30-Day Milestone — ✅ Achieved

> *"Find the document where I discussed Formula Student."*

AIOS searches indexed documents, identifies relevant files, summarizes findings, and answers from content — all running fully offline. Demonstrated with real documents including IELTS results, nutrition guides, and project notes.

---

## Architecture


aios/

├── crates/

│   ├── shared        # Core types and structs

│   ├── memory        # SQLite database layer + migrations

│   ├── document      # File parsing and chunking (PDF, DOCX, TXT)

│   ├── embeddings    # Vector generation via Ollama

│   ├── search        # Tantivy keyword + cosine similarity search

│   ├── models        # Ollama LLM client with streaming

│   ├── assistant     # Orchestration — context, memory, search injection

│   ├── system        # Linux system integration (planned)

│   ├── voice         # whisper.cpp voice interface (planned)

│   ├── api           # HTTP API layer (planned)

│   ├── ui-tui        # Terminal REPL (current)

│   └── ui-desktop    # Tauri + egui desktop UI (planned)


---

## How It Works

```
You ask a question
       ↓
Load memories from past sessions
       ↓
Embed query → cosine similarity over stored vectors
       ↓
Tantivy keyword search over indexed chunks
       ↓
Merge results → top 5 most relevant chunks
       ↓
Inject chunks + memories into system prompt
       ↓
Stream response from local LLM (Ollama)
       ↓
Extract new facts → store as memories
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust |

| Async runtime | Tokio |

| LLM backend | Ollama (llama3.2) |

| Embeddings | Ollama (nomic-embed-text) |

| Database | SQLite via rusqlite |

| Full-text search | Tantivy |

| Voice | whisper.cpp (planned) |

| Terminal UI | ratatui (planned) |

| Desktop UI | Tauri + egui (planned) |

---

## Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Ollama](https://ollama.com)

---

## Getting Started

**1. Clone the repo**
```bash
git clone https://github.com/yourusername/aios.git
cd aios
```

**2. Pull the required models**
```bash
ollama pull llama3.2
ollama pull nomic-embed-text
```

**3. Configure environment**
```bash
cp .env.example .env
```

**4. Run**
```bash
cargo run -p aios-ui-tui
```

---

## Usage


> hello                                    # chat with the assistant
> /index /path/to/file.pdf                 # index a single file
> /index-dir /path/to/folder               # index a directory
> /embed <document-uuid>                   # embed a document's chunks
> /memories                                # list what AIOS knows about you
> /remember key = value                    # store a fact manually
> /forget key                              # delete a memory
> /quit                                    # exit


---

## Vision

AIOS is the first step toward an assistant-first operating system. The long-term goal is a full Linux distribution where the AI assistant is the primary interface — always running, always learning, always aware of your work. No desktop, no file manager, no application launcher. Just you and your system, in conversation.

Think Gideon from The Flash. Think JARVIS before the suit.

---

## License

MIT
EOF
