#!/usr/bin/env python3
"""Isolated real-server fixture for Playwright. Never opens the operator database."""
import base64, hashlib, json, os, secrets, signal, sqlite3, subprocess, tempfile, time
from pathlib import Path
from urllib.request import urlopen
root = Path(__file__).resolve().parents[1]
(root/'web/test-results/session.json').unlink(missing_ok=True)
with tempfile.TemporaryDirectory(prefix='traces-ui-') as temp:
    database = Path(temp) / 'traces.sqlite3'
    env = {**os.environ, 'BIND_ADDR': '127.0.0.1:4173', 'PUBLIC_BASE_URL': 'http://127.0.0.1:4173', 'DATABASE_PATH': str(database), 'BACKUP_DIR': str(Path(temp)/'backups'), 'STATIC_DIR': str(root/'web/build'), 'ALLOWED_EMAILS': 'tester@example.com', 'GITHUB_CLIENT_ID': '', 'GITHUB_CLIENT_SECRET': '', 'TRACE_INGEST_TOKEN': '', 'RUST_LOG': 'agent_traces=warn'}
    binary = root / 'target/debug/agent-traces'
    server = subprocess.Popen([str(binary)], cwd=temp, env=env)
    def shutdown(*_):
        server.terminate()
    signal.signal(signal.SIGTERM, shutdown)
    signal.signal(signal.SIGINT, shutdown)
    try:
        for _ in range(150):
            if server.poll() is not None: raise RuntimeError('Fixture server exited')
            try:
                if urlopen('http://127.0.0.1:4173/api/health').status == 200: break
            except OSError: time.sleep(.1)
        now = int(time.time()*1000)
        conn = sqlite3.connect(database)
        conn.execute('PRAGMA foreign_keys=ON')
        for t in range(122):
            giant = t == 121
            trace = 'ui-giant' if giant else f'ui-trace-{t:03}'
            workflow = 'Long research run' if giant else ['Research assistant','Customer support','Document extraction','Code review'][t%4]
            raw = {'object':'trace','id':trace,'workflow_name':workflow,'metadata':{'environment':'test'}}
            conn.execute('INSERT INTO traces(id,workflow_name,raw,first_seen,last_seen) VALUES(?,?,?,?,?)',(trace,workflow,json.dumps(raw),now-t*60000,now-t*60000))
            for i in range(10000 if giant else 4):
                span = f'{trace}-step-{i:05}'
                name = 'Research assistant' if i==0 else 'search_documents' if i==1 else 'Generate response'
                data = {'type':'agent' if i==0 else 'function' if i==1 else 'generation','name':name,'model':'model-a' if i>1 else None,'input':[{'role':'user','content':'Find the latest research on efficient inference.'}], 'output':[{'role':'assistant','content':'The findings highlight three improvements: batch requests, reuse cached results, and measure latency under load.'}], 'usage':{'input_tokens':420,'output_tokens':180}}
                error = {'message':'The search provider timed out. Please retry.','code':'timeout'} if not giant and t%5==0 and i==1 else None
                raw = {'object':'trace.span','id':span,'trace_id':trace,'parent_id':f'{trace}-step-00000' if i else None,'span_data':data,'error':error,'started_at':'2026-09-12T10:00:00Z','ended_at':'2026-09-12T10:00:01Z'}
                if i==3: raw['span_data']['output'].append({'role':'assistant','content':'<img src=x onerror="window.__injected=true"> literal payload'})
                conn.execute('INSERT INTO spans(id,trace_id,parent_id,span_type,name,model,started_at,ended_at,duration_ms,has_error,error_text,input_tokens,output_tokens,raw) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)',(span,trace,raw['parent_id'],data['type'],name,data['model'],now-5000+i,now-4000+i,1000,int(error is not None),json.dumps(error) if error else '',420,180,json.dumps(raw)))
            conn.execute('UPDATE traces SET started_at=?,ended_at=? WHERE id=?',(now-5000,now+6000 if giant else now-2000,trace))
        conn.execute("INSERT INTO trace_search(rowid,trace_id,workflow_name,group_id,models,errors) SELECT t.rowid,t.id,t.workflow_name,t.group_id,(SELECT GROUP_CONCAT(DISTINCT model) FROM spans WHERE trace_id=t.id),(SELECT GROUP_CONCAT(DISTINCT NULLIF(error_text,'')) FROM spans WHERE trace_id=t.id) FROM traces t")
        token = secrets.token_urlsafe(32)
        digest = base64.urlsafe_b64encode(hashlib.sha256(token.encode()).digest()).rstrip(b'=').decode()
        conn.execute('INSERT INTO sessions(hash,github_id,login,email,expires_at) VALUES(?,?,?,?,?)',(digest,1,'Ty026','tester@example.com',now+86400000))
        conn.commit(); conn.close()
        state = root/'web/test-results/session.json'
        state.parent.mkdir(parents=True,exist_ok=True)
        state.write_text(json.dumps({'token':token}))
        print('Isolated UI fixture ready',flush=True)
        server.wait()
    finally:
        if server.poll() is None:
            server.terminate()
            try: server.wait(timeout=10)
            except subprocess.TimeoutExpired: server.kill(); server.wait()
