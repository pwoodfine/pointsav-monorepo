# NEXT.md — app-workplace-workbench

> Last updated: 2026-05-27
> Read at session start. Update before session end.

---

## Foundation (complete 2026-05-27)

- [x] src-tauri/ Tauri v1.7 skeleton with `get_workbench_url` / `set_workbench_port` IPC
- [x] src/index.html loads configured URL via __TAURI__ invoke
- [x] tauri.conf.json: minimumSystemVersion 10.13, CSP allows localhost
- [x] README.md + README.es.md, CLAUDE.md, package.json

## Wave 1 — active

- [ ] Copy icons from `app-workplace-memo/src-tauri/icons/` before first build
- [ ] Run `npm install && npm run build` on macOS 10.13; verify binary opens
- [ ] First-run port configuration screen: show a setup dialog if `workbench-config.json` is absent
- [ ] Graceful error page when workbench server is not reachable (retry button + port change link)
- [ ] Update window title to show connected URL and connection status
- [ ] Smoke test: workbench loads over WireGuard PPN (`http://10.8.0.9:<port>`)
- [ ] Add to project-software `binary-targets.yaml` once first build passes

## Pending decisions

- [ ] Confirm actual port used by app-privategit-workbench (currently assumed 3000)
- [ ] Determine if `app-privategit-workbench` will run as a system service or manual launch
