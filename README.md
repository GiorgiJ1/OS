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
| 1 | Core assistant loop + Ollama integration | ✅ Done |
| 2 | Document ingestion (PDF, DOCX, TXT) | ✅ Done |
| 3 | Embedding engine (nomic-embed-text) | ✅ Done |
| 4 | Semantic search (Tantivy + cosine similarity) | 🔄 Next |
| 5 | Context injection into prompts | ⬜ Planned |
| 6 | Memory system | ⬜ Planned |
| 7 | Linux system integration | ⬜ Planned |
| 8 | Voice interface (whisper.cpp) | ⬜ Planned |
| 9 | Terminal UI (ratatui) + Desktop UI (Tauri) | ⬜ Planned |
| 10 | Linux distribution packaging | ⬜ Planned |

---

## 30-Day Milestone

> *"Find the document where I discussed Formula Student."*

AIOS should search indexed documents, identify relevant files, summarize findings, and open the selected document. This single demo proves local AI integration, file indexing, semantic search, and real-world usefulness — all running offline.

---

## Architecture
aios/

├── crates/

│   ├── shared        # Core types and structs

│   ├── memory        # SQLite database layer + migrations

│   ├── document      # File parsing and chunking (PDF, DOCX, TXT)

│   ├── embeddings    # Vector generation via Ollama

│   ├── search        # Tantivy keyword + semantic search

│   ├── models        # Ollama LLM client with streaming

│   ├── assistant     # Orchestration layer

│   ├── system        # Linux system integration

│   ├── voice         # whisper.cpp voice interface

│   ├── api           # HTTP API layer

│   ├── ui-tui        # Terminal interface (ratatui)

│   └── ui-desktop    # Desktop interface (Tauri + egui)

## Tech Stack

| Layer | Technology |
|-------|-----------|

| Language | Rust |

| Async runtime | Tokio |

| LLM backend | Ollama (llama3.2) |

| Embeddings | Ollama (nomic-embed-text) |

| Database | SQLite via rusqlite |

| Search | Tantivy |

| Voice | whisper.cpp |

| Terminal UI | ratatui |

| Desktop UI | Tauri + egui |

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

```
> hello                                    # chat with the assistant
> /index /path/to/file.pdf                 # index a single file
> /index-dir /path/to/folder               # index an entire directory
> /embed <document-uuid>                   # embed a document's chunks
> /quit                                    # exit
```

---

## Vision

AIOS is the first step toward an assistant-first operating system. The long-term goal is a full Linux distribution where the AI assistant is the primary interface — no desktop, no file manager, no application launcher. Just you and your system, in conversation.

---

## License

MIT
EOF

echo "README.md created"
