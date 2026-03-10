# Synapse

A thin CLI shell wrapping the [Synaptic](https://github.com/synaptic-ai/synaptic) agent framework.

## Quick Start

```bash
# Build
cargo build --release

# Copy and edit the config
cp synaptic.toml.example synaptic.toml
export OPENAI_API_KEY="sk-..."

# Interactive REPL
./target/release/synapse

# Single-shot mode
./target/release/synapse "What is the capital of France?"
```

## Usage

```
synapse [MESSAGE]          Send a message and exit (single-shot mode)
synapse                    Start interactive REPL
synapse --session <ID>     Resume an existing session
synapse --list-sessions    List all sessions
synapse -m <MODEL>         Override model from config
synapse -c <FILE>          Specify config file path
```

## REPL Commands

| Command      | Description               |
|-------------|---------------------------|
| `/quit`     | Exit the REPL             |
| `/exit`     | Exit the REPL             |
| `/session`  | Show current session info |
| `/sessions` | List all sessions         |

## Configuration

Synapse looks for configuration in this order:

1. Path specified with `-c` flag
2. `./synaptic.toml` in the current directory
3. `~/.synaptic/config.toml`

See `synaptic.toml.example` for the full configuration format.

## Session Persistence

Each conversation is stored as a session with a unique ID. Sessions are persisted
as JSONL transcripts in the configured `sessions_dir` (default: `.sessions/`).
Resume any session with `--session <ID>`.
