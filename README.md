<img width="1000" height="120" alt="banner" src="https://github.com/user-attachments/assets/5cc01e55-2aca-4f10-82e2-0ac8b93ea055" />
<svg width="100%" height="120" viewBox="0 0 1000 120" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <style>
      .bg { fill: #0d0d1f; }
      .title { font-family: 'Courier New', monospace; font-weight: bold; fill: #e0e0ff; }
      .subtitle { font-family: 'Courier New', monospace; fill: #534AB7; }
      .duck-group {
        animation: walk 8s linear infinite;
      }
      @keyframes walk {
        0%   { transform: translateX(-60px); }
        100% { transform: translateX(1060px); }
      }
      .bob {
        animation: bob 0.6s ease-in-out infinite alternate;
      }
      @keyframes bob {
        from { transform: translateY(0px); }
        to   { transform: translateY(-3px); }
      }
      rect.px { shape-rendering: crispEdges; }
    </style>
  </defs>

  <rect class="bg" width="1000" height="120" rx="12"/>

  <text x="40" y="50" class="title" font-size="28">Skvanchi</text>
  <text x="40" y="75" class="subtitle" font-size="14">AI-native operating layer for Linux — always on, always watching, always yours</text>

  <g class="duck-group">
    <g class="bob" transform="translate(0,70) scale(2.2)">
      <rect class="px" x="6" y="0" width="4" height="1" fill="#F5C518"/>
      <rect class="px" x="5" y="1" width="6" height="1" fill="#F5C518"/>
      <rect class="px" x="4" y="2" width="8" height="1" fill="#F5C518"/>
      <rect class="px" x="4" y="3" width="3" height="1" fill="#F5C518"/>
      <rect class="px" x="7" y="3" width="1" height="1" fill="#1a1a1a"/>
      <rect class="px" x="8" y="3" width="4" height="1" fill="#F5C518"/>
      <rect class="px" x="4" y="4" width="8" height="1" fill="#F5C518"/>
      <rect class="px" x="12" y="4" width="3" height="1" fill="#E07B10"/>
      <rect class="px" x="3" y="5" width="9" height="1" fill="#F5C518"/>
      <rect class="px" x="12" y="5" width="3" height="1" fill="#E07B10"/>
      <rect class="px" x="3" y="6" width="3" height="1" fill="#F5C518"/>
      <rect class="px" x="6" y="6" width="3" height="1" fill="#FFF8DC"/>
      <rect class="px" x="9" y="6" width="3" height="1" fill="#F5C518"/>
      <rect class="px" x="3" y="7" width="3" height="1" fill="#F5C518"/>
      <rect class="px" x="6" y="7" width="3" height="1" fill="#FFF8DC"/>
      <rect class="px" x="9" y="7" width="3" height="1" fill="#F5C518"/>
      <rect class="px" x="4" y="8" width="8" height="1" fill="#F5C518"/>
      <rect class="px" x="5" y="9" width="6" height="1" fill="#F5C518"/>
      <rect class="px" x="5" y="10" width="6" height="1" fill="#F5C518"/>
      <rect class="px" x="6" y="11" width="4" height="1" fill="#F5C518"/>
      <rect class="px" x="6" y="12" width="1" height="1" fill="#E07B10"/>
      <rect class="px" x="9" y="12" width="1" height="1" fill="#E07B10"/>
      <rect class="px" x="6" y="13" width="3" height="1" fill="#E07B10"/>
      <rect class="px" x="9" y="13" width="3" height="1" fill="#E07B10"/>
    </g>
  </g>
</svg>






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
