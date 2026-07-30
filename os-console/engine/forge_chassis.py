import os, glob

woodfine_dir = "/opt/woodfine"
active_archives = 0
if os.path.exists(woodfine_dir):
    active_archives = len([d for d in os.listdir(woodfine_dir) if os.path.isdir(os.path.join(woodfine_dir, d)) and d.startswith('cluster-totebox-')])

if active_archives == 0: active_archives = 1

html_out = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Command Ledger | PointSav Digital Systems</title>
    <style>
        :root {{ 
            --wf-canvas: #F9FAFB; 
            --wf-card: #FFFFFF; 
            --wf-text: #1F2937; 
            --wf-slate: #4B5563;
            --wf-muted: #6B7280; 
            --wf-accent: #164679; 
            --wf-border: #E5E7EB; 
            --wf-border-heavy: #374151; 
            --wf-alert: #DC2626; 
            --font-mono: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace;
            --font-sans: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{ background-color: var(--wf-canvas); color: var(--wf-text); font-family: var(--font-mono); height: 100vh; display: flex; flex-direction: column; overflow: hidden; }}
        header {{ flex-shrink: 0; border-bottom: 2px solid var(--wf-border-heavy); padding: 12px 24px; display: flex; justify-content: space-between; font-size: 12px; text-transform: uppercase; letter-spacing: 0.5px; background: var(--wf-card); }}
        .hdr-label {{ color: var(--wf-muted); margin-right: 8px; }}
        .hdr-val {{ color: var(--wf-accent); font-weight: bold; }}
        .workspace {{ display: flex; flex-grow: 1; overflow: hidden; }}
        .left-ribbon {{ width: 20%; flex-shrink: 0; background: var(--wf-card); border-right: 2px solid var(--wf-border-heavy); display: flex; flex-direction: column; padding: 24px; overflow-y: auto; }}
        .federation-box {{ margin-bottom: 24px; padding: 16px; border: 1px solid var(--wf-border-heavy); background: var(--wf-canvas); }}
        .fed-title {{ font-size: 11px; color: var(--wf-slate); margin-bottom: 12px; font-weight: bold; letter-spacing: 0.5px; border-bottom: 1px dashed var(--wf-border-heavy); padding-bottom: 4px; text-transform: uppercase; }}
        .fed-stat {{ font-size: 11px; display: flex; justify-content: space-between; margin-bottom: 8px; }}
        .fed-stat span.ok {{ color: #059669; font-weight: bold; }}
        .f-key-list {{ display: flex; flex-direction: column; gap: 8px; margin-bottom: auto; }}
        .f-key {{ background: var(--wf-canvas); border: 1px solid var(--wf-muted); border-radius: 4px; padding: 10px 12px; font-size: 11px; font-weight: bold; color: var(--wf-text); cursor: pointer; user-select: none; transition: all 0.1s; text-align: left; text-transform: uppercase; letter-spacing: 0.5px;}}
        .f-key:hover {{ border-color: var(--wf-accent); background: #F3F4F6; }}
        .f-key.active-key {{ background: var(--wf-accent); color: #fff; border-color: var(--wf-accent); transform: translateX(4px); box-shadow: -2px 2px 0 var(--wf-border-heavy); }}
        .resizer {{ width: 4px; background-color: var(--wf-border-heavy); cursor: col-resize; flex-shrink: 0; z-index: 10; transition: background-color 0.2s; }}
        .resizer:hover {{ background-color: var(--wf-accent); }}
        .cartridge-viewport {{ flex-grow: 1; display: flex; overflow: hidden; background: var(--wf-canvas); }}
        .omnibar {{ flex-shrink: 0; border-top: 2px solid var(--wf-border-heavy); padding: 16px 24px; display: flex; align-items: center; background: var(--wf-card); }}
        .prompt-prefix {{ color: var(--wf-accent); margin-right: 12px; font-weight: bold; font-size: 14px; }}
        #cli-input {{ background: transparent; border: none; color: var(--wf-text); font-family: var(--font-mono); font-size: 14px; flex-grow: 1; outline: none; }}
    </style>
</head>
<body>
    <header>
        <div><span class="hdr-label">NODE:</span><span class="hdr-val">console.pointsav.com</span></div>
        <div><span class="hdr-label">AUTH:</span><span class="hdr-val">MACHINE-BASED (MBA)</span></div>
        <div><span class="hdr-label">HUD:</span><span class="hdr-val">FEDERATED PANOPTICON V1.8.1</span></div>
    </header>
    
    <div class="workspace">
        <div class="left-ribbon">
            <div class="federation-box">
                <div class="fed-title">MESH TELEMETRY</div>
                <div class="fed-stat"><span>ACTIVE ARCHIVES:</span> <span class="ok" id="archive-count">{active_archives}</span></div>
                <div class="fed-stat"><span>GLOBAL SYNC:</span> <span class="ok">UDP ZERO-BROKER</span></div>
                <div class="fed-stat"><span>NETWORK STATE:</span> <span class="ok">SECURE</span></div>
            </div>
            <div class="f-key-list" id="hardware-ribbon"></div>
        </div>
        <div class="resizer" id="main-resizer"></div>
        <div class="cartridge-viewport" id="cartridge-viewport">
            <div style="width: 100%; display: flex; align-items: center; justify-content: center; color: var(--wf-muted); font-size: 14px; text-transform: uppercase; letter-spacing: 1px;">[ AWAITING F-KEY STRIKE ]</div>
        </div>
    </div>
    
    <div class="omnibar"><span class="prompt-prefix" id="ui-prompt">operator@chassis:~$</span><input type="text" id="cli-input" autocomplete="off" spellcheck="false" autofocus></div>
    
    <script>
        const viewport = document.getElementById('cartridge-viewport');
        const input = document.getElementById('cli-input');
        const uiPrompt = document.getElementById('ui-prompt');
        const ribbon = document.getElementById('hardware-ribbon');
        const F_KEY_MAP = {{
            'F1': {{ name: 'app-console-help', label: '[F1] HELP' }},
            'F2': {{ name: 'app-console-people', label: '[F2] PEOPLE' }},
            'F3': {{ name: 'app-console-email', label: '[F3] EMAIL' }},
            'F4': {{ name: 'app-console-content', label: '[F4] CONTENT' }},
            'F5': {{ name: 'app-console-vault', label: '[F5] FILE SYSTEM' }},
            'F12': {{ name: 'app-console-input', label: '[F12] INPUT MACHINE' }}
        }};
        Object.keys(F_KEY_MAP).forEach(key => {{
            const btn = document.createElement('div');
            btn.className = 'f-key'; btn.id = `btn-${{key}}`; btn.textContent = F_KEY_MAP[key].label;
            btn.onclick = () => mountCartridge(key);
            ribbon.appendChild(btn);
        }});

        async function mountCartridge(key) {{
            const cartridge = F_KEY_MAP[key];
            document.querySelectorAll('.f-key').forEach(b => b.classList.remove('active-key'));
            document.getElementById(`btn-${{key}}`).classList.add('active-key');
            uiPrompt.textContent = `operator@${{cartridge.name.split('-')[2]}}:~$`;
            viewport.innerHTML = `<div style="width: 100%; display: flex; align-items: center; justify-content: center; color: var(--wf-muted); font-size: 14px; text-transform: uppercase;">[ FETCHING SECURE PAYLOAD... ]</div>`;
            try {{
                const response = await fetch(`/cartridges/${{cartridge.name}}.html?v=` + Date.now());
                if (response.ok) {{
                    viewport.innerHTML = await response.text();
                    const scripts = viewport.querySelectorAll('script');
                    scripts.forEach(oldScript => {{
                        const newScript = document.createElement('script');
                        Array.from(oldScript.attributes).forEach(attr => newScript.setAttribute(attr.name, attr.value));
                        newScript.appendChild(document.createTextNode(oldScript.innerHTML));
                        oldScript.parentNode.replaceChild(newScript, oldScript);
                    }});
                }} else {{ viewport.innerHTML = `<div style="width: 100%; display: flex; align-items: center; justify-content: center; color: var(--wf-alert); font-weight: bold;">[ FAULT: CARTRIDGE MISSING ]</div>`; }}
            }} catch (err) {{ viewport.innerHTML = `<div style="width: 100%; display: flex; align-items: center; justify-content: center; color: var(--wf-alert); font-weight: bold;">[ FAULT: ISOLATION BOUNDARY BLOCK ]</div>`; }}
            input.focus();
        }}
        document.addEventListener('keydown', function(e) {{ if (F_KEY_MAP[e.key]) {{ e.preventDefault(); mountCartridge(e.key); }} }});
        document.addEventListener('click', (e) => {{ if (!e.target.closest('.left-ribbon') && e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA' && e.target.tagName !== 'SELECT') {{ input.focus(); }} }});
    </script>
</body>
</html>
"""
with open("/var/www/os-console/index.html", "w") as f:
    f.write(html_out)
