# Changing C0RE without rebuilding the .exe

There are two halves. Pick the fast loop for the half you're changing.

## Dashboard (anything in `web/`): live reload on your Mac

The dashboard runs on your Mac and uses the real C0RE on the PC for its data. Save a
file and the browser updates within a second. Nothing on the PC changes.

Once (needs Node.js 22+, `brew install node`):

```bash
cd .\web\
npm install
```

Each time:

```bash
npm run dev
```

Open <http://localhost:5173/admin/> and log in with your dashboard password. Tailscale must
be connected, since the API calls go to `http://kihurn-desktop:7080`. To use another C0RE,
start it with `C0RE_URL=http://127.0.0.1:7080 npm run dev`.

Checks before you hand changes back: `npm run check` (types) and `npm test`.

## Gateway (anything in `src/`): run from source on the PC

Needs Rust on the PC once: install from <https://rustup.rs>, then open a new PowerShell.

```powershell
Stop-ScheduledTask -TaskName C0RE        # free port 7080
cd path\to\c0re
cd web; npm install; npm run build; cd ..  # the .exe embeds the built dashboard
cargo run --release -- serve              # Ctrl+C to stop
```

The first build takes a few minutes; later ones take seconds. It uses the same keys,
settings and database as the installed copy. When you're done:
`Start-ScheduledTask -TaskName C0RE`.

`cargo test` runs the gateway's tests.

## Making the real install

When everything's right, ask for (or run) `bash scripts/build-release.sh` (on Linux/WSL
with the Windows cross-compiler), then update the PC as in `setup-windows.md`.
