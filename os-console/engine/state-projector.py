import os, json, html, re, glob

TOTEBOX = "/opt/woodfine/cluster-totebox-alpha-1"
WWW_ROOT = "/var/www/os-console/cartridges"

def project_f1():
    html_out = """
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; padding: 40px; overflow-y: auto; background: var(--wf-card);">
        <h2 style="margin: 0; font-family: var(--font-sans); font-size: 24px; color: var(--wf-accent); letter-spacing: 0.5px; border-bottom: 2px solid var(--wf-border-heavy); padding-bottom: 16px; margin-bottom: 32px;">Operator Manual: Panopticon V1.8.1</h2>
        <div style="max-width: 800px; font-family: var(--font-sans); font-size: 15px; line-height: 1.8; color: var(--wf-text);">
            <h3 style="color: var(--wf-text); font-family: var(--font-sans); font-size: 18px; margin-bottom: 12px; border-left: 4px solid var(--wf-accent); padding-left: 12px;">1. The Immutable Ledger & WORM Protocol</h3>
            <p style="margin-bottom: 32px; color: var(--wf-slate);">The Host OS serves as the Operator's ephemeral scratchpad. The <strong>Totebox Archive</strong> is an Immutable Cryptographic Ledger. Data is formally ingested through strict Domain Mandates.</p>
            
            <h3 style="color: var(--wf-text); font-family: var(--font-sans); font-size: 18px; margin-bottom: 12px; border-left: 4px solid var(--wf-accent); padding-left: 12px;">2. Bootable Enclaves (Quantum Collapse)</h3>
            <p style="margin-bottom: 32px; color: var(--wf-slate);">Totebox Archives are not standard compressed folders. When extracted from the mesh, a specific Sovereign Archive collapses into a <strong>Self-Executing Bootable Image (.ISO/.IMG)</strong>.</p>
            
            <h3 style="color: var(--wf-text); font-family: var(--font-sans); font-size: 18px; margin-bottom: 12px; border-left: 4px solid var(--wf-accent); padding-left: 12px;">3. Parametric Compute (F4)</h3>
            <p style="margin-bottom: 32px; color: var(--wf-slate);">The Compute Matrix eliminates prompt engineering. The Operator defines the Linguistic Domain (COMM, MEMO, LEGAL) and the desired Depth. External API costs are structurally blocked unless explicitly authorized.</p>
        </div>
    </div>
    """
    with open(os.path.join(WWW_ROOT, "app-console-help.html"), "w") as f: f.write(html_out)

def project_f2():
    ledger_path = os.path.join(TOTEBOX, "service-people/substrate/ledger_personnel.jsonl")
    html_out = """
    <style>
        .f2-row { transition: background-color 0.1s; border-bottom: 1px solid var(--wf-border); font-family: var(--font-sans); font-size: 14px; }
        .f2-row:nth-child(even) { background-color: var(--wf-canvas); }
        .f2-row:hover { background-color: #E5E7EB !important; cursor: pointer; }
        .f2-row.active { border-left: 4px solid var(--wf-accent); background-color: #EFF6FF !important; }
        .index-card { background: var(--wf-card); border: 1px solid var(--wf-border-heavy); padding: 40px; box-shadow: 4px 4px 0px rgba(0,0,0,0.05); position: relative; border-radius: 4px; }
        .inspector-label { font-size: 11px; color: var(--wf-muted); font-weight: bold; margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.5px;}
        .data-block { margin-bottom: 24px; }
        .inspector-data { font-family: var(--font-sans); font-size: 15px; color: var(--wf-text); word-break: break-all; display: flex; align-items: center; justify-content: space-between;}
        .copy-btn { background: var(--wf-canvas); border: 1px solid var(--wf-border); color: var(--wf-slate); font-family: var(--font-sans); font-size: 12px; padding: 6px 12px; cursor: pointer; transition: all 0.1s; font-weight: 600; border-radius: 4px; }
        .copy-btn:hover { border-color: var(--wf-accent); color: var(--wf-accent); background: #fff; }
        .table-header-banner { background: var(--wf-border-heavy); color: #fff; font-weight: bold; padding: 10px 12px; font-size: 11px; text-transform: uppercase; letter-spacing: 1px; display: flex; justify-content: space-between; border-radius: 4px 4px 0 0; }
    </style>
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; padding: 32px; overflow-y: hidden;">
        <div style="flex-shrink: 0; border-bottom: 2px solid var(--wf-border); padding-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 24px;">
            <h2 style="margin: 0; font-size: 18px; text-transform: uppercase; letter-spacing: 1px;">PEOPLE (CRM DIRECTORY)</h2>
            <div style="font-family: var(--font-mono); font-size: 11px; color: var(--wf-muted); font-weight: bold;"><span style="color: var(--wf-accent);">OMNIBAR FILTER ACTIVE</span></div>
        </div>
        <div style="display: flex; gap: 32px; flex: 1; min-height: 0; width: 100%;">
            <div style="flex: 0 0 45%; max-width: 45%; display: flex; flex-direction: column; border: 1px solid var(--wf-border); border-radius: 4px;">
                <div class="table-header-banner"><span>[ PRIMARY LEDGER: ENTITIES ]</span><span style="color: #9CA3AF;">WORM PROTOCOL</span></div>
                <div style="overflow-y: auto; flex: 1; background: #fff;">
                    <table style="width: 100%; border-collapse: collapse; table-layout: fixed;">
                        <tr style="background: var(--wf-canvas); color: var(--wf-slate); text-align: left; position: sticky; top: 0; z-index: 10; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px;">
                            <th style="padding: 12px 16px; width: 35%; border-bottom: 1px solid var(--wf-border);">NAME</th>
                            <th style="padding: 12px 16px; width: 30%; border-bottom: 1px solid var(--wf-border);">LOCATION</th>
                            <th style="padding: 12px 16px; width: 35%; border-bottom: 1px solid var(--wf-border);">DOMAIN</th>
                        </tr>
    """
    rows_html = ""
    if os.path.exists(ledger_path):
        with open(ledger_path, 'r') as f:
            for i, line in enumerate(reversed(f.readlines())):
                if not line.strip(): continue
                if "david g. johnston" in line.lower() or "david.g.johnston" in line.lower(): continue
                try:
                    data = json.loads(line)
                    raw_anchor = str(data.get('identity_anchor', 'Unknown'))
                    clean_name = raw_anchor
                    clean_email = data.get('email', '')
                    match = re.search(r'(.*?)<([^>]+)>', raw_anchor)
                    if match:
                        clean_name = match.group(1).strip().strip('"').strip("'")
                        if not clean_email: clean_email = match.group(2).strip()
                            
                    clean_name = clean_name.title()
                    clean_email = clean_email.lower()
                    clean_loc = str(data.get('location', 'Unverified')).title()
                    domain = "External Affairs"
                    archetype = "The Envoy"
                    if "woodfine" in clean_email.lower(): 
                        domain = "Internal Command"
                        archetype = "The Guardian"
                    date_str = str(data.get('date', 'Unknown Date'))[:16]
                    subject_str = str(data.get('latest_subject', 'No activity recorded'))
                    s_name, s_email, s_loc, s_domain, s_arch, s_date, s_subj = html.escape(clean_name), html.escape(clean_email), html.escape(clean_loc), html.escape(domain), html.escape(archetype), html.escape(date_str), html.escape(subject_str)
                    
                    rows_html += """<tr class="f2-row" data-name="{0}" data-email="{1}" data-loc="{2}" data-chart="{3}" data-arch="{4}" data-date="{5}" data-subj="{6}">
                        <td style="padding: 12px 16px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; color: var(--wf-text);">{0}</td>
                        <td style="padding: 12px 16px; color: var(--wf-slate); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{2}</td>
                        <td style="padding: 12px 16px; font-family: var(--font-sans); color: var(--wf-accent); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{3}</td>
                    </tr>""".format(s_name, s_email, s_loc, s_domain, s_arch, s_date, s_subj)
                except: continue
    if not rows_html: rows_html = """<tr><td colspan="3" style="padding: 32px; text-align: center; color: var(--wf-muted); font-family: var(--font-sans);">Directory is empty</td></tr>"""
    html_out += rows_html
    html_out += """
                    </table>
                </div>
            </div>
            <div style="flex: 0 0 calc(55% - 32px); max-width: calc(55% - 32px); display: flex; flex-direction: column; overflow-y: auto;">
                <div id="inspector-empty" style="text-align: center; padding: 80px 20px; color: var(--wf-muted); font-family: var(--font-sans); font-size: 14px; border: 2px dashed var(--wf-border); border-radius: 4px;">
                    Select a record to view index card
                </div>
                <div id="inspector-data" class="index-card" style="display: none;">
                    <div style="position: absolute; top: 16px; right: 24px; font-size: 11px; color: var(--wf-muted); font-family: var(--font-mono); text-transform: uppercase;">[ ENTITY TOKEN ]</div>
                    <div style="display: flex; align-items: center; justify-content: space-between; border-bottom: 2px solid var(--wf-border); padding-bottom: 20px; margin-bottom: 24px;">
                        <h3 id="insp-name" style="margin: 0; color: var(--wf-accent); font-size: 28px; font-family: var(--font-sans); font-weight: 700;">Name</h3>
                        <button class="copy-btn" onclick="copyValue('insp-name', this)">Copy Name</button>
                    </div>
                    <div class="data-block"><div class="inspector-label">Known Email Address</div><div class="inspector-data"><span id="insp-email" style="color: var(--wf-accent); font-family: var(--font-mono); font-size: 14px;">...</span><button class="copy-btn" onclick="copyValue('insp-email', this)">Copy Email</button></div></div>
                    <div class="data-block"><div class="inspector-label">Geographic Location</div><div class="inspector-data" id="insp-loc">...</div></div>
                    <div class="data-block"><div class="inspector-label">Chart of Accounts & Archetype</div><div class="inspector-data"><span id="insp-chart">...</span> <span style="background: var(--wf-canvas); color: var(--wf-slate); padding: 4px 8px; border-radius: 4px; font-size: 12px; margin-left: 12px; border: 1px solid var(--wf-border);" id="insp-arch">...</span></div></div>
                    <div style="margin-top: 40px; padding-top: 24px; border-top: 1px dashed var(--wf-border);">
                        <div class="data-block"><div class="inspector-label">Last Spoke (Temporal Anchor)</div><div class="inspector-data" id="insp-date" style="color: var(--wf-slate); font-family: var(--font-mono); font-size: 13px;">...</div></div>
                        <div class="data-block"><div class="inspector-label">Latest Event Hook</div><div class="inspector-data" id="insp-subj" style="color: var(--wf-text); font-size: 14px;">...</div></div>
                    </div>
                    <div style="position: absolute; bottom: 16px; right: 24px; font-size: 11px; color: #059669; font-family: var(--font-mono); font-weight: bold;">SOURCE: TBX-ALPHA-1</div>
                </div>
            </div>
        </div>
    </div>
    <script>
        (function() {
            document.querySelectorAll('.f2-row').forEach(row => {
                row.addEventListener('click', function() {
                    document.querySelectorAll('.f2-row').forEach(r => r.classList.remove('active'));
                    this.classList.add('active');
                    document.getElementById('inspector-empty').style.display = 'none';
                    document.getElementById('inspector-data').style.display = 'block';
                    document.getElementById('insp-name').textContent = this.getAttribute('data-name');
                    document.getElementById('insp-email').textContent = this.getAttribute('data-email');
                    document.getElementById('insp-loc').textContent = this.getAttribute('data-loc');
                    document.getElementById('insp-chart').textContent = this.getAttribute('data-chart');
                    document.getElementById('insp-arch').textContent = this.getAttribute('data-arch');
                    document.getElementById('insp-date').textContent = this.getAttribute('data-date');
                    document.getElementById('insp-subj').textContent = this.getAttribute('data-subj');
                });
            });
            window.copyValue = function(elementId, btnElement) {
                const textToCopy = document.getElementById(elementId).textContent;
                navigator.clipboard.writeText(textToCopy).then(() => {
                    const originalText = btnElement.textContent;
                    btnElement.textContent = "Copied ✓"; btnElement.style.background = "#059669"; btnElement.style.color = "#fff"; btnElement.style.borderColor = "#059669";
                    setTimeout(() => { btnElement.textContent = originalText; btnElement.style.background = "var(--wf-canvas)"; btnElement.style.color = "var(--wf-slate)"; btnElement.style.borderColor = "var(--wf-border)"; }, 1500);
                });
            };
            const omnibar = document.getElementById('cli-input');
            const rows = document.querySelectorAll('.f2-row');
            if (window.currentOmnibarListener) { omnibar.removeEventListener('input', window.currentOmnibarListener); }
            window.currentOmnibarListener = function(e) {
                const searchTerm = e.target.value.toLowerCase().replace('search ', '').replace('search', '').trim();
                rows.forEach(row => { row.style.display = row.textContent.toLowerCase().includes(searchTerm) ? '' : 'none'; });
            };
            omnibar.addEventListener('input', window.currentOmnibarListener);
            if(!omnibar.value.startsWith('search')) { omnibar.value = 'search '; }
        })();
    </script>
    """
    with open(os.path.join(WWW_ROOT, "app-console-people.html"), "w") as f: f.write(html_out)

def project_f3():
    ledger_path = os.path.join(TOTEBOX, "service-email/substrate/ledger_emails.jsonl")
    html_out = """
    <style>
        .f3-row { transition: background-color 0.1s; border-bottom: 1px solid var(--wf-border); font-family: var(--font-sans); font-size: 14px; }
        .f3-row:nth-child(even) { background-color: var(--wf-canvas); }
        .f3-row:hover { background-color: #E5E7EB !important; cursor: pointer; }
        .f3-row.active { border-left: 4px solid var(--wf-accent); background-color: #EFF6FF !important; }
        .action-bar { display: flex; gap: 12px; background: var(--wf-canvas); padding: 12px 24px; border-bottom: 1px solid var(--wf-border); border-radius: 4px 4px 0 0; }
        .action-btn { background: #fff; border: 1px solid var(--wf-border); padding: 8px 16px; font-family: var(--font-sans); font-size: 13px; font-weight: 600; cursor: pointer; transition: all 0.1s; border-radius: 4px; color: var(--wf-text);}
        .action-btn:hover { background: var(--wf-canvas); border-color: var(--wf-slate); }
        .action-btn.primary { background: var(--wf-accent); color: #fff; border-color: var(--wf-accent); }
        .action-btn.primary:hover { background: #1e3a8a; }
        .copy-btn { background: var(--wf-canvas); border: 1px solid var(--wf-border); color: var(--wf-slate); font-family: var(--font-sans); font-size: 11px; padding: 4px 8px; cursor: pointer; transition: all 0.1s; margin-left: 12px; border-radius: 4px; font-weight: 600;}
        .copy-btn:hover { border-color: var(--wf-accent); color: var(--wf-accent); background: #fff;}
        .attach-pill { background: #E5E7EB; border: 1px solid var(--wf-border); padding: 6px 12px; font-size: 12px; font-family: var(--font-sans); margin-right: 8px; border-radius: 16px; color: var(--wf-text); font-weight: 500; }
        .table-header-banner { background: var(--wf-border-heavy); color: #fff; font-weight: bold; padding: 10px 12px; font-size: 11px; text-transform: uppercase; letter-spacing: 1px; display: flex; justify-content: space-between; border-radius: 4px 4px 0 0;}
    </style>
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; padding: 32px; overflow-y: hidden;">
        <div style="flex-shrink: 0; border-bottom: 2px solid var(--wf-border); padding-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 24px;">
            <h2 style="margin: 0; font-size: 18px; text-transform: uppercase; letter-spacing: 1px;">EMAIL (EVENT LEDGER)</h2>
            <div style="font-family: var(--font-mono); font-size: 11px; color: var(--wf-muted); font-weight: bold;"><span style="color: #059669;">[ 3RD DERIVATIVE DATA ]</span> | <span style="color: var(--wf-accent);">OMNIBAR FILTER ACTIVE</span></div>
        </div>
        <div style="display: flex; gap: 32px; flex: 1; min-height: 0; width: 100%;">
            <div style="flex: 0 0 40%; max-width: 40%; display: flex; flex-direction: column; border: 1px solid var(--wf-border); border-radius: 4px;">
                <div class="table-header-banner"><span>[ PRIMARY LEDGER: EVENTS ]</span><span style="color: #9CA3AF;">WORM PROTOCOL</span></div>
                <div style="overflow-y: auto; flex: 1; background: #fff;">
                    <table style="width: 100%; border-collapse: collapse; table-layout: fixed;">
                        <tr style="background: var(--wf-canvas); color: var(--wf-slate); text-align: left; position: sticky; top: 0; z-index: 10; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px;">
                            <th style="padding: 12px 16px; width: 35%; border-bottom: 1px solid var(--wf-border);">SENDER</th>
                            <th style="padding: 12px 16px; width: 45%; border-bottom: 1px solid var(--wf-border);">SUBJECT</th>
                            <th style="padding: 12px 16px; width: 20%; text-align: right; border-bottom: 1px solid var(--wf-border);">DATE</th>
                        </tr>
    """
    rows_html = ""
    bodies_html = "" 
    if os.path.exists(ledger_path):
        with open(ledger_path, 'r') as f:
            for i, line in enumerate(reversed(f.readlines())):
                if not line.strip(): continue
                if "david g. johnston" in line.lower() or "david.g.johnston" in line.lower(): continue
                try:
                    data = json.loads(line)
                    s_id = "eml_" + str(i)
                    s_subj = html.escape(data.get("subject", "No Subject"))
                    s_name = html.escape(data.get("sender_name", "Unknown")).title()
                    s_email = html.escape(data.get("sender_email", "")).lower()
                    s_date = html.escape(data.get("date", ""))[:16]
                    s_body = html.escape(data.get("body_text", ""))
                    s_vault = "TBX-ALPHA-1"
                    s_attach = html.escape(json.dumps(data.get("attachments", [])))
                    
                    rows_html += """<tr class="f3-row" data-id="{0}" data-subj="{1}" data-name="{2}" data-email="{3}" data-date="{4}" data-vault="{5}" data-attach="{6}">
                        <td style="padding: 12px 16px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; color: var(--wf-text);">{2}</td>
                        <td style="padding: 12px 16px; color: var(--wf-slate); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{1}</td>
                        <td style="padding: 12px 16px; text-align: right; font-family: var(--font-mono); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--wf-slate);">{4}</td>
                    </tr>""".format(s_id, s_subj, s_name, s_email, s_date, s_vault, s_attach)
                    bodies_html += "<div id=\"body_" + s_id + "\" style=\"display:none;\">" + s_body + "</div>\n"
                except: continue
    if not rows_html: rows_html = """<tr><td colspan="3" style="padding: 32px; text-align: center; color: var(--wf-muted); font-family: var(--font-sans);">Event ledger is empty</td></tr>"""
    html_out += rows_html
    html_out += """
                    </table>
                </div>
            </div>
            <div style="flex: 0 0 calc(60% - 32px); max-width: calc(60% - 32px); display: flex; flex-direction: column; background: #fff; border: 1px solid var(--wf-border); border-radius: 4px; box-shadow: 4px 4px 0px rgba(0,0,0,0.03); position: relative;">
                <div id="reader-empty" style="flex: 1; display: flex; align-items: center; justify-content: center; color: var(--wf-muted); font-family: var(--font-sans); font-size: 14px;">
                    Select an event to read
                </div>
                <div id="reader-data" style="display: none; flex-direction: column; height: 100%; min-width: 0;">
                    <div style="position: absolute; top: 16px; right: 24px; font-size: 11px; color: #059669; font-family: var(--font-mono); font-weight: bold;" id="read-vault">TBX-ALPHA-1</div>
                    <div class="action-bar" style="flex-shrink: 0; padding-right: 140px;">
                        <button class="action-btn primary" onclick="replyFormat(this)">Forward / Reply to Clipboard</button>
                        <button class="action-btn" onclick="copyRaw(this)">Copy Raw Text</button>
                    </div>
                    <div style="padding: 32px 32px 24px 32px; border-bottom: 1px solid var(--wf-border); background: #fff; flex-shrink: 0;">
                        <h3 id="read-subj" style="margin-top: 0; font-size: 24px; font-family: var(--font-sans); color: var(--wf-text); margin-bottom: 20px; word-break: break-word; line-height: 1.4;">Subject</h3>
                        <div style="display: flex; flex-direction: column; gap: 12px; font-size: 14px; font-family: var(--font-sans); color: var(--wf-slate);">
                            <div style="display: flex; align-items: center; justify-content: space-between;">
                                <div style="display: flex; align-items: center;"><strong style="color: var(--wf-text); margin-right: 8px;">From:</strong> <span id="read-name" style="font-weight: 500; margin-right: 8px; color: var(--wf-accent);">...</span><button class="copy-btn" onclick="copyValue('read-name', this)">Copy</button></div>
                                <div style="font-family: var(--font-mono); color: var(--wf-muted); font-size: 12px;" id="read-date">...</div>
                            </div>
                            <div style="display: flex; align-items: center;"><strong style="color: var(--wf-text); margin-right: 8px;">Email:</strong> <span id="read-email" style="font-family: var(--font-mono); font-size: 13px;">...</span><button class="copy-btn" onclick="copyValue('read-email', this)">Copy</button></div>
                            <div style="margin-top: 16px; border-top: 1px dashed var(--wf-border); padding-top: 16px;" id="read-attachments">No attachments</div>
                        </div>
                    </div>
                    <div id="read-body" style="flex: 1; padding: 32px; overflow-y: auto; font-family: var(--font-sans); font-size: 15px; line-height: 1.8; color: var(--wf-text); white-space: pre-wrap; overflow-wrap: break-word; word-break: break-word; background: var(--wf-canvas);">...</div>
                </div>
            </div>
        </div>
        <div id="hidden-email-bodies">
    """
    html_out += bodies_html
    html_out += """
        </div>
    </div>
    <script>
        (function() {
            window.currentEmailId = null;
            document.querySelectorAll('.f3-row').forEach(row => {
                row.addEventListener('click', function() {
                    document.querySelectorAll('.f3-row').forEach(r => r.classList.remove('active'));
                    this.classList.add('active');
                    document.getElementById('reader-empty').style.display = 'none';
                    document.getElementById('reader-data').style.display = 'flex';
                    window.currentEmailId = this.getAttribute('data-id');
                    document.getElementById('read-subj').textContent = this.getAttribute('data-subj');
                    document.getElementById('read-name').textContent = this.getAttribute('data-name');
                    document.getElementById('read-email').textContent = this.getAttribute('data-email');
                    document.getElementById('read-date').textContent = this.getAttribute('data-date');
                    document.getElementById('read-vault').textContent = "SOURCE: " + this.getAttribute('data-vault');
                    
                    const attachData = JSON.parse(this.getAttribute('data-attach') || '[]');
                    const attachContainer = document.getElementById('read-attachments');
                    if (attachData.length > 0) { attachContainer.innerHTML = attachData.map(a => "<span class='attach-pill'>📎 " + a + "</span>").join(''); } 
                    else { attachContainer.innerHTML = '<span style="color: var(--wf-muted); font-size: 13px;">[ No attachments ]</span>'; }
                    
                    const rawText = document.getElementById('body_' + window.currentEmailId).textContent;
                    document.getElementById('read-body').textContent = rawText;
                });
            });
            window.copyValue = function(elementId, btnElement) {
                const textToCopy = document.getElementById(elementId).textContent;
                navigator.clipboard.writeText(textToCopy).then(() => {
                    const originalText = btnElement.textContent;
                    btnElement.textContent = "Copied ✓"; btnElement.style.background = "#059669"; btnElement.style.color = "#fff"; btnElement.style.borderColor = "#059669";
                    setTimeout(() => { btnElement.textContent = originalText; btnElement.style.background = "var(--wf-canvas)"; btnElement.style.color = "var(--wf-slate)"; btnElement.style.borderColor = "var(--wf-border)"; }, 1500);
                });
            };
            window.replyFormat = function(btn) {
                if(!window.currentEmailId) return;
                const sender = document.getElementById('read-name').textContent;
                const date = document.getElementById('read-date').textContent;
                const body = document.getElementById('read-body').textContent;
                const lines = body.split('\\n');
                const quotedBody = lines.map(line => '> ' + line).join('\\n');
                const clipboardPayload = "On " + date + ", " + sender + " wrote:\\n\\n" + quotedBody;
                navigator.clipboard.writeText(clipboardPayload).then(() => {
                    const orig = btn.textContent;
                    btn.textContent = "Formatted to Clipboard ✓"; btn.style.background = "#059669"; btn.style.color = "#fff"; btn.style.borderColor = "#059669";
                    setTimeout(() => { btn.textContent = orig; btn.style.background = "var(--wf-accent)"; }, 2000);
                });
            };
            window.copyRaw = function(btn) {
                const body = document.getElementById('read-body').textContent;
                navigator.clipboard.writeText(body).then(() => {
                    const orig = btn.textContent;
                    btn.textContent = "Raw Copied ✓"; btn.style.background = "#059669"; btn.style.color = "#fff"; btn.style.borderColor = "#059669";
                    setTimeout(() => { btn.textContent = orig; btn.style.background = "#fff"; btn.style.color = "var(--wf-text)"; btn.style.borderColor = "var(--wf-border)"; }, 2000);
                });
            };
            const omnibar = document.getElementById('cli-input');
            const rows = document.querySelectorAll('.f3-row');
            if (window.currentOmnibarListener) { omnibar.removeEventListener('input', window.currentOmnibarListener); }
            window.currentOmnibarListener = function(e) {
                const searchTerm = e.target.value.toLowerCase().replace('search ', '').replace('search', '').trim();
                rows.forEach(row => { row.style.display = row.textContent.toLowerCase().includes(searchTerm) ? '' : 'none'; });
            };
            omnibar.addEventListener('input', window.currentOmnibarListener);
            if(!omnibar.value.startsWith('search')) { omnibar.value = 'search '; }
        })();
    </script>
    """
    with open(os.path.join(WWW_ROOT, "app-console-email.html"), "w") as f: f.write(html_out)

def project_f4():
    html_out = """
    <style>
        .forge-btn { background: #fff; border: 1px solid var(--wf-border); padding: 10px 16px; font-family: var(--font-sans); font-size: 13px; font-weight: 600; cursor: pointer; transition: all 0.1s; border-radius: 4px; color: var(--wf-text);}
        .forge-btn:hover { background: var(--wf-canvas); border-color: var(--wf-slate); }
        .forge-btn.primary { background: var(--wf-accent); color: #fff; border-color: var(--wf-accent); }
        .forge-btn.primary:hover { background: #1e3a8a; }
        .domain-btn { display: block; width: 100%; text-align: left; background: #fff; border: 1px solid var(--wf-border); padding: 12px 16px; font-family: var(--font-sans); font-size: 14px; font-weight: 500; color: var(--wf-slate); margin-bottom: 8px; cursor: pointer; transition: all 0.1s; border-radius: 4px;}
        .domain-btn:hover { border-color: var(--wf-accent); background: var(--wf-canvas); }
        .domain-btn.active { background: var(--wf-accent); color: #fff; border-color: var(--wf-accent); font-weight: 600; box-shadow: -2px 2px 0 var(--wf-border-heavy); transform: translateX(4px); }
        .toggle-switch { position: relative; display: inline-block; width: 44px; height: 24px; }
        .toggle-switch input { opacity: 0; width: 0; height: 0; }
        .slider { position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0; background-color: var(--wf-border-heavy); transition: .2s; border-radius: 24px; }
        .slider:before { position: absolute; content: ""; height: 16px; width: 16px; left: 4px; bottom: 4px; background-color: white; transition: .2s; border-radius: 50%; }
        input:checked + .slider { background-color: var(--wf-alert); }
        input:checked + .slider:before { transform: translateX(20px); }
        .economics-panel { background: #fff; border: 1px solid var(--wf-border); padding: 24px; margin-bottom: 32px; border-left: 4px solid var(--wf-accent); border-radius: 4px; box-shadow: 2px 2px 0 rgba(0,0,0,0.02);}
        .economics-panel.billable { border-left-color: var(--wf-alert); background: #FEF2F2;}
    </style>
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; padding: 32px; overflow-y: auto;">
        <div style="flex-shrink: 0; border-bottom: 2px solid var(--wf-border); padding-bottom: 16px; margin-bottom: 24px; display: flex; justify-content: space-between; align-items: flex-end;">
            <h2 style="margin: 0; font-size: 18px; text-transform: uppercase; letter-spacing: 1px;">CONTENT (COMPUTE MATRIX)</h2>
            <div style="font-family: var(--font-mono); font-size: 11px; font-weight: bold;">
                ACTIVE ROUTING: <span id="compute-telemetry" style="color: #059669;">[ LOCAL SLM / ZERO COST ]</span>
            </div>
        </div>
        <div style="display: flex; gap: 32px; flex: 1;">
            <div style="flex: 0 0 35%; border-right: 1px solid var(--wf-border); padding-right: 32px; display: flex; flex-direction: column;">
                <div id="eco-panel" class="economics-panel">
                    <div style="font-size: 13px; font-family: var(--font-sans); font-weight: bold; margin-bottom: 16px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--wf-text);">Compute Economics</div>
                    <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; border-bottom: 1px dashed var(--wf-border); padding-bottom: 16px;">
                        <span style="font-family: var(--font-mono); font-size: 12px; font-weight: bold; color: var(--wf-slate);">EXTERNAL API (BILLABLE)</span>
                        <label class="toggle-switch"><input type="checkbox" id="api-toggle" onchange="toggleCompute()"><span class="slider"></span></label>
                    </div>
                    <div id="eco-status" style="font-family: var(--font-sans); font-size: 14px; color: var(--wf-slate); line-height: 1.6;">
                        System locked to Local SLM.<br>Zero external data transmission.
                    </div>
                </div>
                <div style="margin-top: 8px;">
                    <div style="font-size: 13px; font-family: var(--font-sans); font-weight: bold; color: var(--wf-text); margin-bottom: 16px; text-transform: uppercase; letter-spacing: 0.5px; border-bottom: 1px solid var(--wf-border); padding-bottom: 8px;">Linguistic Domain</div>
                    <button class="domain-btn active" id="dom-COMM" onclick="setDomain('COMM')">COMM (Communications)</button>
                    <button class="domain-btn" id="dom-MEMO" onclick="setDomain('MEMO')">MEMO (Structural Logic)</button>
                    <button class="domain-btn" id="dom-LEGAL" onclick="setDomain('LEGAL')">LEGAL (Fact Extraction)</button>
                    <button class="domain-btn" id="dom-TRANSLATE" onclick="setDomain('TRANSLATE')">TRANSLATE</button>
                    <button class="domain-btn" id="dom-QUERY" onclick="setDomain('QUERY')">QUERY (Database)</button>
                </div>
            </div>
            <div style="flex: 1; display: flex; flex-direction: column;">
                <div id="action-bar-depth" style="display: flex; gap: 12px; margin-bottom: 16px; flex-wrap: wrap;">
                    <button class="forge-btn" onclick="executeCompute('L1: PROOFREAD')">L1: Proofread</button>
                    <button class="forge-btn" onclick="executeCompute('L2: COPY EDIT')">L2: Copy Edit</button>
                    <button class="forge-btn primary" onclick="executeCompute('L3: GENERATE CONTENT')">L3: Generate Content</button>
                </div>
                <div id="action-bar-execute" style="display: none; gap: 12px; margin-bottom: 16px; flex-wrap: wrap;">
                    <button class="forge-btn primary" onclick="executeCompute('EXECUTE COMMAND')">Execute Command</button>
                </div>
                <textarea id="forge-editor" placeholder="Draft input matrix here..." style="flex-grow: 1; padding: 32px; border: 1px solid var(--wf-border); border-radius: 4px; font-family: var(--font-sans); font-size: 15px; line-height: 1.8; color: var(--wf-text); resize: none; outline: none; background: #fff; box-shadow: inset 0 2px 4px rgba(0,0,0,0.02);"></textarea>
            </div>
        </div>
    </div>
    <script>
        (function() {
            window.currentDomain = 'COMM';
            window.setDomain = function(domain) {
                window.currentDomain = domain;
                document.querySelectorAll('.domain-btn').forEach(btn => btn.classList.remove('active'));
                document.getElementById('dom-' + domain).classList.add('active');
                if(domain === 'TRANSLATE' || domain === 'QUERY') {
                    document.getElementById('action-bar-depth').style.display = 'none';
                    document.getElementById('action-bar-execute').style.display = 'flex';
                } else {
                    document.getElementById('action-bar-depth').style.display = 'flex';
                    document.getElementById('action-bar-execute').style.display = 'none';
                }
            };
            window.toggleCompute = function() {
                const toggle = document.getElementById('api-toggle');
                const panel = document.getElementById('eco-panel');
                const status = document.getElementById('eco-status');
                const telemetry = document.getElementById('compute-telemetry');
                if(toggle.checked) {
                    panel.classList.add('billable');
                    status.innerHTML = "<span style='color: var(--wf-alert); font-weight: bold;'>WARNING: Hyperscaler Authorized.</span><br>L2/L3 execution will transmit data to External API.<br>Cost metrics actively monitored.";
                    telemetry.innerHTML = "<span style='color: var(--wf-alert);'>[ GEMINI CLI / BILLABLE ]</span>";
                } else {
                    panel.classList.remove('billable');
                    status.innerHTML = "System locked to Local SLM.<br>Zero external data transmission.";
                    telemetry.innerHTML = "<span style='color: #059669;'>[ LOCAL SLM / ZERO COST ]</span>";
                }
            };
            window.executeCompute = function(depth) {
                const editor = document.getElementById('forge-editor');
                const toggle = document.getElementById('api-toggle').checked;
                if(!editor.value.trim()) {
                    editor.value = "[ SYSTEM FAULT: No raw mass provided for computation. ]";
                    setTimeout(() => { editor.value = ""; }, 2000);
                    return;
                }
                if ((depth.includes('L2') || depth.includes('L3') || window.currentDomain === 'TRANSLATE') && !toggle) {
                    const orig = editor.value;
                    editor.value = "[ ACCESS DENIED ]\\n\\nCompute Depth [" + depth + "] on Domain [" + window.currentDomain + "] requires External Hyperscaler API.\\n\\nPlease authorize billable access via the toggle switch to proceed.\\n\\n---\\n" + orig;
                    return;
                }
                const originalText = editor.value;
                const engine = toggle ? "GEMINI CLI API" : "LOCAL SLM";
                editor.value = "Establishing compute tunnel to " + engine + "...\\nDomain: " + window.currentDomain + "\\nExecuting Depth: " + depth + "...\\nCompiling...\\n";
                setTimeout(() => {
                    editor.value = "========================================================\\n COMPUTE COMPLETED: " + depth + "\\n DOMAIN APPLIED: " + window.currentDomain + "\\n ENGINE: " + engine + "\\n========================================================\\n\\n[ Deterministic Output compiled successfully ]\\n\\nOriginal Matrix:\\n" + originalText;
                }, 1500);
            };
        })();
    </script>
    """
    with open(os.path.join(WWW_ROOT, "app-console-content.html"), "w") as f: f.write(html_out)

def project_f5():
    # ... Keeping F5 (File System) largely monospace as it represents physical directory structures.
    # Softening the borders and padding.
    woodfine_dir = "/opt/woodfine"
    def build_ascii_tree(dir_path, prefix=''):
        try:
            contents = [d for d in os.listdir(dir_path) if not d.startswith('.') and d != '__pycache__']
        except: return ""
        contents.sort()
        pointers = ['├── '] * (len(contents) - 1) + ['└── '] if contents else []
        out = ""
        for pointer, path in zip(pointers, contents):
            full_path = os.path.join(dir_path, path)
            out += prefix + pointer + path + "\\n"
            if os.path.isdir(full_path):
                extension = '│   ' if pointer == '├── ' else '    '
                out += build_ascii_tree(full_path, prefix=prefix+extension)
        return out

    tree_str = ""
    archive_options = ""
    if os.path.exists(woodfine_dir):
        archives = [d for d in os.listdir(woodfine_dir) if os.path.isdir(os.path.join(woodfine_dir, d)) and d.startswith('cluster-totebox-')]
        archives.sort()
        for arch in archives:
            full_path = os.path.join(woodfine_dir, arch)
            tree_str += "[ ONLINE ] /opt/woodfine/" + arch + "/\\n"
            tree_str += build_ascii_tree(full_path)
            tree_str += "\\n"
            archive_options += "<option value='" + arch + "'>" + arch + "</option>"
            
    if not tree_str: tree_str = "[ SYSTEM FAULT: NO ARCHIVES FOUND ON GCP NODE ]"
    if not archive_options: archive_options = "<option value='none'>NO ARCHIVES DETECTED</option>"
    
    html_out = """
    <style>
        .export-select { width: 100%; padding: 12px 16px; border: 1px solid var(--wf-border); border-radius: 4px; font-family: var(--font-sans); font-size: 14px; margin-bottom: 24px; background: #fff; outline: none; }
        .export-select:focus { border-color: var(--wf-accent); }
    </style>
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; gap: 20px; padding: 32px; overflow-y: auto;">
        <div style="flex-shrink: 0; border-bottom: 2px solid var(--wf-border); padding-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end;">
            <h2 style="margin: 0; font-size: 18px; text-transform: uppercase; letter-spacing: 1px;">FILE SYSTEM (QUANTUM TOPOGRAPHY)</h2>
            <div style="font-family: var(--font-mono); font-size: 11px; color: var(--wf-muted); font-weight: bold;">
                VERIFICATION: <span style="color: #059669;">FEDERATED CRAWLER ACTIVE</span>
            </div>
        </div>
        <div style="display: flex; gap: 32px; flex: 1;">
            <div style="flex: 1.2; border-right: 1px solid var(--wf-border); padding-right: 32px; display: flex; flex-direction: column;">
                <div style="font-size: 13px; font-family: var(--font-sans); font-weight: bold; color: var(--wf-slate); border-bottom: 1px dashed var(--wf-border); padding-bottom: 12px; margin-bottom: 16px; display: flex; justify-content: space-between; text-transform: uppercase; letter-spacing: 0.5px;">
                    <span>The Substrate (Live Federated State)</span>
                    <span style="color: #059669; font-family: var(--font-mono);">ACTIVE READ</span>
                </div>
                <pre style="font-family: var(--font-mono); font-size: 13px; color: var(--wf-text); line-height: 1.6; margin: 0; overflow-y: auto; overflow-x: auto; background: #fff; padding: 24px; border: 1px solid var(--wf-border); border-radius: 4px; flex-grow: 1;">
""" + tree_str + """</pre>
            </div>
            <div style="flex: 0.8; display: flex; flex-direction: column;">
                <div style="font-size: 13px; font-family: var(--font-sans); font-weight: bold; color: var(--wf-slate); border-bottom: 1px dashed var(--wf-border); padding-bottom: 12px; margin-bottom: 16px; text-transform: uppercase; letter-spacing: 0.5px;">
                    The Quantum Anchor (Local State)
                </div>
                <div style="background: #fff; padding: 32px; border: 1px solid var(--wf-border); border-radius: 4px; box-shadow: 2px 2px 0px rgba(0,0,0,0.02); font-family: var(--font-sans); font-size: 14px; line-height: 1.8;">
                    <div style="margin-bottom: 20px;"><strong>Mesh State:</strong> <span style="color: var(--wf-accent); font-family: var(--font-mono); font-size: 12px; font-weight: bold;">[ ASYMMETRIC / ENTANGLED ]</span></div>
                    <div style="margin-bottom: 20px; color: var(--wf-slate);"><strong>Master Merkle Root:</strong><br><span style="font-size: 12px; font-family: var(--font-mono); color: var(--wf-text);">FEDERATION_ROOT_0x8F4A2C9B3E...</span></div>
                    <div style="margin-bottom: 24px; color: var(--wf-text);">
                        The Semantic Substrate (Left) is mathematically entangled with your offline physical vaults. You may forcefully collapse a specific dimension into a singular, transferrable micro-kernel environment.
                    </div>
                    <div style="border-top: 1px dashed var(--wf-border); margin-top: 32px; padding-top: 32px;">
                        <div style="font-size: 12px; font-weight: bold; color: var(--wf-alert); margin-bottom: 12px; text-transform: uppercase; letter-spacing: 0.5px;">1. Select Sovereign Target:</div>
                        <select id="export-archive-target" class="export-select">
                            """ + archive_options + """
                        </select>
                        <div style="font-size: 12px; font-weight: bold; color: var(--wf-text); margin-bottom: 12px; text-transform: uppercase; letter-spacing: 0.5px;">2. Compile Cryptographic Image:</div>
                        <button id="export-btn" style="width: 100%; background: var(--wf-alert); color: #fff; border: none; border-radius: 4px; padding: 16px; font-family: var(--font-sans); font-weight: 600; cursor: pointer; text-transform: uppercase; letter-spacing: 1px; transition: opacity 0.2s;" onclick="simulateExport()">
                            Export Bootable Image (.ISO)
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </div>
    <script>
        function simulateExport() {
            const target = document.getElementById('export-archive-target').value;
            const btn = document.getElementById('export-btn');
            const orig = btn.innerHTML;
            btn.innerHTML = "<span style='font-size: 12px;'>[ ISOLATING: " + target + " ]</span><br>COMPILING MICRO-KERNEL...";
            btn.style.background = "#059669";
            setTimeout(() => {
                btn.innerHTML = "Export Complete<br><span style='font-size: 11px; font-weight: normal; font-family: var(--font-mono);'>" + target + ".iso ready</span>";
                setTimeout(() => {
                    btn.innerHTML = orig;
                    btn.style.background = "var(--wf-alert)";
                }, 3000);
            }, 2000);
        }
    </script>
    """
    with open(os.path.join(WWW_ROOT, "app-console-vault.html"), "w") as f: f.write(html_out)

def project_f12():
    html_out = """
    <style>
        .ingress-select { width: 100%; padding: 12px 16px; border: 1px solid var(--wf-border); border-radius: 4px; font-family: var(--font-sans); font-size: 14px; margin-bottom: 32px; background: #fff; outline: none; }
        .ingress-select:focus { border-color: var(--wf-accent); }
        .drop-zone { border: 2px dashed var(--wf-border-heavy); background: #fff; border-radius: 4px; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 300px; cursor: pointer; transition: all 0.2s; }
        .drop-zone:hover { background: var(--wf-canvas); border-color: var(--wf-accent); }
        .preview-box { background: #fff; border: 1px solid var(--wf-accent); border-radius: 4px; padding: 24px; font-family: var(--font-mono); font-size: 13px; font-weight: bold; color: var(--wf-accent); text-align: center; margin-top: 32px; }
        .step-label { font-size: 13px; font-family: var(--font-sans); font-weight: bold; color: var(--wf-text); margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.5px; }
        .step-desc { font-size: 13px; font-family: var(--font-sans); color: var(--wf-slate); margin-bottom: 16px; }
    </style>
    <div style="width: 100%; height: 100%; display: flex; flex-direction: column; padding: 32px; overflow-y: auto;">
        <div style="flex-shrink: 0; border-bottom: 2px solid var(--wf-border); padding-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 32px;">
            <h2 style="margin: 0; font-size: 18px; text-transform: uppercase; letter-spacing: 1px;">INPUT MACHINE (SEMANTIC INGRESS)</h2>
            <div style="font-family: var(--font-mono); font-size: 11px; color: var(--wf-muted); font-weight: bold;">
                ROUTING PROTOCOL: <span style="color: #059669;">STRICT MANDATE</span>
            </div>
        </div>
        
        <div style="display: flex; gap: 40px; flex: 1;">
            <div style="flex: 0 0 40%; display: flex; flex-direction: column; border-right: 1px solid var(--wf-border); padding-right: 40px;">
                <div class="step-label">1. Target Archive (Node)</div>
                <div class="step-desc">Select the federated destination node.</div>
                <select id="sel-archive" class="ingress-select" onchange="updatePreview()">
                    <option value="corporate-1">cluster-totebox-corporate-1</option>
                    <option value="personnel-1">cluster-totebox-alpha-1</option>
                    <option value="property-1">cluster-totebox-property-1</option>
                </select>

                <div class="step-label">2. Target Service (Domain)</div>
                <div class="step-desc">Select the Immutable Vault Domain.</div>
                <select id="sel-service" class="ingress-select" onchange="updatePreview()">
                    <option value="study">service-study (Ephemeral Sandbox)</option>
                    <option value="research">service-research (External Data)</option>
                    <option value="minutebook">service-minutebook (Legal/Corp)</option>
                    <option value="bookkeeping">service-bookkeeping (Financial)</option>
                    <option value="content">service-content (Linguistic Rules)</option>
                </select>

                <div class="step-label">3. Chart of Accounts (Taxonomy)</div>
                <div class="step-desc">Select the organizational taxonomy node.</div>
                <select id="sel-domain" class="ingress-select" onchange="updatePreview()">
                    <option value="General_Study">General_Study</option>
                    <option value="Corporate_Governance">Corporate_Governance</option>
                    <option value="External_Affairs">External_Affairs</option>
                    <option value="Projects">Projects</option>
                    <option value="Financial_Ledgers">Financial_Ledgers</option>
                </select>
                
                <div style="margin-top: auto; padding: 24px; background: #fff; border: 1px solid var(--wf-border); border-radius: 4px; font-family: var(--font-sans); font-size: 13px; color: var(--wf-slate); line-height: 1.6;">
                    <strong style="color: var(--wf-text);">WORM PROTOCOL ACTIVE</strong><br><br>
                    Files cannot be ingested directly into standard directories. Assets must be explicitly bound to a Node, Service, and Domain to generate an absolute cryptographic routing vector.
                </div>
            </div>
            
            <div style="flex: 1; display: flex; flex-direction: column;">
                <div class="step-label">4. Execute Ingress (Drop Asset)</div>
                <div class="drop-zone" onclick="simulateDrop()">
                    <div style="font-size: 48px; margin-bottom: 24px;">📥</div>
                    <div style="font-family: var(--font-sans); font-size: 16px; font-weight: 600; color: var(--wf-text);">Drop base asset here</div>
                    <div style="font-family: var(--font-sans); font-size: 13px; color: var(--wf-muted); margin-top: 8px;">.PDF, .CSV, .EML, .DOCX</div>
                </div>
                <div id="preview-container" class="preview-box">
                    AWAITING DROPDOWN CONFIGURATION...
                </div>
            </div>
        </div>
    </div>
    <script>
        (function() {
            window.updatePreview = function() {
                const archive = document.getElementById('sel-archive').value;
                const service = document.getElementById('sel-service').value;
                const domain = document.getElementById('sel-domain').value;
                const hash = Math.random().toString(36).substring(2, 8).toUpperCase();
                const container = document.getElementById('preview-container');
                container.innerHTML = "<span style='font-family: var(--font-sans); font-size: 12px; color: var(--wf-slate); margin-bottom: 8px; display: block;'>ABSOLUTE ROUTING VECTOR:</span><span style='color: var(--wf-text); font-size: 15px;'>" + archive + "_" + service + "_" + domain + "_ASSET-" + hash + ".ext</span>";
            };
            window.simulateDrop = function() {
                const container = document.getElementById('preview-container');
                container.innerHTML = "<span style='color: #059669; font-family: var(--font-sans); font-size: 15px;'>Executing cryptographic commit...</span>";
                container.style.borderColor = "#059669";
                setTimeout(() => {
                    container.innerHTML = "<span style='color: #059669; font-family: var(--font-sans); font-size: 15px; font-weight: bold;'>Commit Successful.</span><br><span style='font-size: 13px; font-weight: normal; color: var(--wf-slate);'>Asset routed to absolute vector.</span>";
                    setTimeout(() => {
                        container.style.borderColor = "var(--wf-accent)";
                        updatePreview();
                    }, 2000);
                }, 1000);
            };
            updatePreview();
        })();
    </script>
    """
    with open(os.path.join(WWW_ROOT, "app-console-input.html"), "w") as f: f.write(html_out)

if __name__ == "__main__":
    project_f1()
    project_f2()
    project_f3()
    project_f4()
    project_f5()
    project_f12()
    print("[SUCCESS] Engine V1.8.1 (Accessibility Patch) generated.")
