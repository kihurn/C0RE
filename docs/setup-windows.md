# Setting up C0RE on your Windows PC

C0RE sits in front of Ollama on your PC. Projects reach Ollama through C0RE with their own
key, from your Tailscale devices (private) or, if you choose, from the internet (public).

**Before you start:** Ollama is installed and running, with `OLLAMA_HOST` unset (or
`127.0.0.1:11434`) and Tailscale is signed in on the PC. (You did both already.)

## 1. Put the files in place

1. Unzip `c0re-<version>-windows-x64.zip`.
2. Move the `C0RE` folder to `%LOCALAPPDATA%\Programs\C0RE`
   (paste that into File Explorer's address bar). No administrator rights needed there.
3. Open PowerShell in that folder (in File Explorer: **File → Open Windows PowerShell**, or
   right-click the folder background → **Open in Terminal**).

Windows may warn that `c0re.exe` is from an unknown publisher (it isn't code-signed). Choose
**More info → Run anyway**.

Optional — so you can type `c0re` from any folder:

```powershell
[Environment]::SetEnvironmentVariable('Path', $env:Path + ";$env:LOCALAPPDATA\Programs\C0RE", 'User')
```

Open a new PowerShell window afterwards. The examples below assume this, or that you are in
the C0RE folder and type `.\c0re` instead of `c0re`.

## 2. First run

```powershell
c0re admin set-password      # dashboard password, 10+ characters
c0re status                  # engine online? Tailscale address?
```

`c0re status` should say **Engine: online** and show your Tailscale address
(`http://100.94.2.98:7080`).

## 3. Start C0RE automatically

Open PowerShell **as administrator** (right-click → **Run as administrator**), go to the C0RE
folder, and run:

```powershell
c0re install
```

This:

- adds a logon task so C0RE starts in the background whenever you log in (no window);
- adds a firewall rule letting **only Tailscale devices** reach port 7080.

Check it's running: `c0re status`, or open <http://127.0.0.1:7080/health> on the PC — it should
show `{"ok":true}`.

Not running as administrator? The task is still installed, and C0RE tells you the firewall
step was skipped. Run `c0re install` again as administrator to add it.

## 4. Open the dashboard

From your Mac or phone (with Tailscale connected), open:

**<http://kihurn-desktop:7080/admin>**

Log in with the password from step 2. Bookmark it; on a phone, "Add to Home Screen" makes it
open like an app.

## 5. Create a key and try it

In the dashboard: **Keys → New key**, project `lenz`, reach **Private**, then copy the key.
You can see it again later with the eye button on the Keys page (or `c0re keys show <id>`);
it's stored encrypted to your Windows account. Or on the PC:

```powershell
c0re keys create --project lenz --reach private --models qwen3-vl:8b-instruct,qwen3-vl:4b-instruct
```

Test from the Mac's Terminal (replace the key):

```bash
curl http://kihurn-desktop:7080/api/chat \
  -H "Authorization: Bearer c0re_YOUR_KEY" \
  -d '{"model":"qwen3-vl:8b-instruct","stream":false,"messages":[{"role":"user","content":"Say hi"}]}'
```

You should get an answer. The same request without the key returns
`{"error":{"code":"missing_key",…}}`.

Projects use C0RE exactly like Ollama: base URL `http://kihurn-desktop:7080`, plus the header
`Authorization: Bearer <key>`. OpenAI-style clients can use `http://kihurn-desktop:7080/v1`
with the key as their API key.

## 6. The public address (optional)

Only if a project runs somewhere without Tailscale (a hosted website, a friend's app).

**One-time setup in the Tailscale admin console** (login.tailscale.com):

1. **DNS** page → make sure **MagicDNS** is on → enable **HTTPS Certificates**.
2. **Access controls** → allow Funnel for this PC. The easiest way: run `c0re public on`
   once; if Tailscale needs permission, the error message says so and Tailscale usually
   prints a link that enables it for you.

Then turn it on — from **Settings → Public address** in the dashboard, or:

```powershell
c0re public on
```

You'll get an address like `https://kihurn-desktop.<your-tailnet>.ts.net:8443`.

- Only keys with reach **Public** work there. Lenz's private key is refused there by design.
- Public keys get stricter default limits (20 per minute, 500 per day, 1 at a time).
- Turn it off any time: `c0re public off`.

Test from a device **not** on Tailscale (e.g. your phone on mobile data with Tailscale off):
open `https://kihurn-desktop.<your-tailnet>.ts.net:8443/health` → `{"ok":true}`.

## Everyday commands

```powershell
c0re status                       # everything at a glance
c0re keys list                    # all keys (without their secrets)
c0re keys show k_abc123           # one key's secret, to copy into a project
c0re keys off k_abc123            # switch a key off immediately
c0re keys on k_abc123 [--public]  # switch it back on
c0re keys edit k_abc123 --rate 30 --daily 1000
c0re usage --days 7               # who used what
c0re public status
```

## Updating to a new version

1. In PowerShell: `Stop-ScheduledTask -TaskName C0RE`
2. Unzip the new version and copy `c0re.exe` and `c0re-svc.exe` over the old ones in
   `%LOCALAPPDATA%\Programs\C0RE` (your keys and settings live elsewhere and are kept).
3. `Start-ScheduledTask -TaskName C0RE`, then reload the dashboard.

Keys made before 0.2 keep working but can't be shown again; delete and recreate one if you
need to see it.

## Where things live

| What | Where |
|---|---|
| Program | `%LOCALAPPDATA%\Programs\C0RE` (where you put it) |
| Settings | `%LOCALAPPDATA%\C0RE\c0re.toml` |
| Keys and usage log | `%LOCALAPPDATA%\C0RE\c0re.db` (a few MB; capped at 100 MB) |
| Log files | `%LOCALAPPDATA%\C0RE\logs\` (7 days kept) |

## Uninstall

```powershell
c0re public off
c0re uninstall          # as administrator, to also remove the firewall rule
```

Then delete the program folder, and `%LOCALAPPDATA%\C0RE` if you want your keys and history
gone too.

## Troubleshooting

| Problem | Fix |
|---|---|
| `c0re status` says **Engine: OFFLINE** | Open Ollama from the Start menu. |
| C0RE isn't answering after login | Look in `%LOCALAPPDATA%\C0RE\startup-error.txt` — if C0RE couldn't start, the reason is there (also in `logs\`). Or run `c0re serve` in PowerShell to see it directly. |
| Dashboard doesn't load from the Mac | Tailscale connected on both? `c0re status` shows the Tailscale address? Run `c0re install` as administrator (firewall rule). |
| `Cannot listen on … port 7080` (os error 10013 or 10048) | Another program uses the port (AnyDesk, for example, takes 7070). Find it with `Get-Process -Id (Get-NetTCPConnection -LocalPort 7080).OwningProcess`. Change `private_port` in `c0re.toml` and run `c0re install` again as administrator. |
| Requests answer slowly / `% GPU` below 100 on Overview | The model doesn't fit on the GPU. Close GPU-heavy apps; keep contexts at 8K. |
| Lost a key | Delete it and create a new one. |
| Forgot the dashboard password | On the PC: `c0re admin set-password`. |
