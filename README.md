# C0RE

      ___           ___           ___           ___     
     /\  \         /\  \         /\  \         /\  \    
    /::\  \       /::\  \       /::\  \       /::\  \   
   /:/\:\  \     /:/\:\  \     /:/\:\  \     /:/\:\  \  
  /:/  \:\  \   /:/  \:\  \   /::\~\:\  \   /::\~\:\  \ 
 /:/__/ \:\__\ /:/__/ \:\__\ /:/\:\ \:\__\ /:/\:\ \:\__\
 \:\  \  \/__/ \:\  \ /:/  / \/_|::\/:/  / \:\~\:\ \/__/
  \:\  \        \:\  /:/  /     |:|::/  /   \:\ \:\__\  
   \:\  \        \:\/:/  /      |:|\/__/     \:\ \/__/  
    \:\__\        \::/  /       |:|  |        \:\__\    
     \/__/         \/__/         \|__|         \/__/    


A private AI gateway for your own PC. C0RE sits in front of [Ollama](https://ollama.com) and
lets your projects use your GPU from anywhere — each with its own key — without ever
exposing Ollama itself.

- **Two entrances.** Private (your [Tailscale](https://tailscale.com) devices) and an optional
  public address (Tailscale Funnel). Each key is *private*, *public* or *off*.
- **Per-project control.** Allowed models, requests per minute and per day, requests at once,
  a cap on context size so nobody pushes your model off the GPU, and request size limits.
- **Safe by default.** Ollama stays on `127.0.0.1`; model management and the dashboard are
  never reachable through a key or the public address; repeated wrong keys get blocked.
- **Private.** The usage log records who, when, which model and how long — never prompts,
  images or answers. All data stays on the PC.
- **Light.** One small Windows program; a dashboard for your browser and phone; a CLI.

Projects talk to C0RE exactly like Ollama (`/api/chat`, `/api/generate`, `/api/embed`,
`/api/tags`, …) or the OpenAI-style `/v1/chat/completions`, adding
`Authorization: Bearer <key>`.

## Get started

See **[docs/setup-windows.md](docs/setup-windows.md)**.

## Develop

```bash
cd web && npm ci && npm run build && cd ..   # dashboard (embedded into the binary)
cargo test                                    # 125 tests, fake Ollama, no GPU needed
cargo run -- serve                            # run locally (C0RE_HOME=./tmp to keep data local)
bash scripts/build-release.sh                 # Windows zip + source zip + git bundle
```

Design and plan: `docs/superpowers/specs/2026-10-30-c0re-design.md`,
`docs/superpowers/plans/2026-10-30-c0re.md`. Dashboard design notes: `web/DESIGN.md`.
