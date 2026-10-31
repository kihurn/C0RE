# C0RE manual test checklist (on the PC)

The automated tests (125 Rust, 25 dashboard) cover key checks, limits, streaming, the usage
log, the admin API and the CLI against a fake Ollama. These checks cover what only the real
PC can show. Note anything that fails with what you did and what you saw.

## Install

- [ ] `c0re.exe` runs (SmartScreen: More info → Run anyway) and `c0re --help` lists the commands.
- [ ] `c0re admin set-password` accepts a 10+ character password and refuses a short one.
- [ ] `c0re status` shows Engine online, the Tailscale address, Public off.
- [ ] **`c0re install` as administrator completes without a PowerShell error**
      (this script could not be syntax-checked before shipping).
- [ ] Task Scheduler shows a task named **C0RE**; Windows Defender Firewall shows the rule
      **C0RE private (Tailscale)** (inbound, TCP 7080, remote 100.64.0.0/10).
- [ ] After signing out and back in, `http://127.0.0.1:7080/health` returns `{"ok":true}`
      without opening anything.
- [ ] Task Manager → Details: `c0re-svc.exe` uses **under 50 MB** of memory when idle.
- [ ] End `c0re-svc.exe` in Task Manager: note whether Task Scheduler restarts it (if not,
      it comes back at the next login; tell me and I'll add a watchdog).
- [ ] Temporarily set `private_port` in `c0re.toml` to a port in use, sign out and in:
      `%LOCALAPPDATA%\C0RE\startup-error.txt` explains why C0RE didn't start. Set it back.

## Private entrance

- [ ] From the Mac, `http://kihurn-desktop:7080/admin` opens the dashboard; login works.
- [ ] Overview shows Engine online, the loaded model with **100% GPU**, and the doors diagram.
- [ ] A curl request from the Mac with a private key streams an answer.
- [ ] The same request without a key → `missing_key`; with a wrong key → `invalid_key`.
- [ ] A request for a model the key isn't allowed → `model_not_allowed`.
- [ ] The request appears in Overview → Recent requests and in `c0re usage`.
- [ ] Switching the key off in the dashboard makes the next request fail with `key_disabled`.
- [ ] From a device **not** on Tailscale (phone, Tailscale off), `http://<PC's home IP>:7080`
      does not connect (firewall).

## Keys shown again (0.2)

- [ ] A key created in 0.2 shows a masked `c0re_••••` with an eye button; the eye shows
      the full key, a second click hides it, and it hides by itself after a minute.
- [ ] The copy button copies the key (if the browser blocks copying on plain http, the key
      is shown with a note to copy it by hand).
- [ ] `c0re keys show <id>` in PowerShell prints the same key.
- [ ] A key made in 0.1 says "Can't be shown"; `c0re keys show` explains why.
- [ ] Open `%LOCALAPPDATA%\C0RE\c0re.db` in Notepad and search for the start of a key:
      not found (it's stored encrypted).

## Dashboard (0.3)

- [ ] Overview's ring shows **GPU LOAD** with temperature and watts; it rises while a model
      is answering and falls back when idle. The boot log has a `gpu rtx 4070 super` line.
- [ ] "VRAM IN USE" matches Task Manager → Performance → GPU → Dedicated GPU memory.
- [ ] Settings and the key editor use sliders; saved values stick after a reload.
- [ ] The footer says `c0re 0.3.0`.

## Public entrance

- [ ] `c0re public on` (or the Settings toggle) gives an `https://…ts.net:8443` URL, or a
      clear message naming the missing Tailscale setting.
- [ ] Dashboard → Settings and Overview both show the public address as On with the URL.
- [ ] From a device not on Tailscale: `/health` on the public URL returns `{"ok":true}`.
- [ ] A **public** key works on the public URL.
- [ ] Lenz's **private** key on the public URL → `key_not_public`.
- [ ] `https://…ts.net:8443/admin` returns 404 (dashboard never public).
- [ ] 10 requests with a wrong key from the same device → the 11th says `too_many_failures`;
      a different device still works.
- [ ] `c0re public off` → the public URL stops answering.

## Failure handling

- [ ] Quit Ollama → requests return `engine_offline`; Overview shows the hollow ring and
      "Engine offline". Reopen Ollama → back to normal without restarting C0RE.
- [ ] Quit Tailscale, restart the PC, start Tailscale later → within ~30 s the private
      entrance answers on the Tailscale address again.
- [ ] Phone browser: dashboard is usable at phone width in light and dark mode.
