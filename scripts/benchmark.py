#!/usr/bin/env python3
"""HTTP mixed-load benchmark against an isolated, already seeded service."""
import argparse, concurrent.futures, datetime, http.client, json, platform, statistics, threading, time
from pathlib import Path
from urllib.parse import urlsplit
p=argparse.ArgumentParser();p.add_argument('--url',default='http://127.0.0.1:4180');p.add_argument('--credentials',required=True);p.add_argument('--output',required=True);p.add_argument('--seconds',type=int,default=30);p.add_argument('--peak-seconds',type=int,default=10);p.add_argument('--database');args=p.parse_args()
creds=json.loads(Path(args.credentials).read_text());base=urlsplit(args.url)
def call(path,method='GET',data=None,ingest=False):
    conn=http.client.HTTPConnection(base.hostname,base.port,timeout=30)
    headers={'Content-Type':'application/json'}
    headers.update({'Authorization':'Bearer '+creds['key']} if ingest else {'Cookie':'traces_session='+creds['session']})
    start=time.perf_counter();conn.request(method,path,body=data,headers=headers);response=conn.getresponse();body=response.read();latency=(time.perf_counter()-start)*1000;conn.close()
    if response.status!=200: raise RuntimeError(f'{path}: {response.status} {body[:100]}')
    return json.loads(body),latency
queries={'recent':'/api/traces','errors':'/api/traces?status=errors','type':'/api/traces?spanType=function','model':'/api/traces?model=model-a','workflow':'/api/traces?q=Research','error_text':'/api/traces?q=timeout','no_match':'/api/traces?q=no-such-workflow-928734','time_range':f'/api/traces?since={int(time.time()*1000)-86400000}','detail':'/api/traces/trace_00000000','tree':'/api/traces/trace_00000000/spans','body':'/api/traces/trace_00000000/spans/span_000000001'}
latencies={k:[] for k in queries};write_latencies=[];errors=[];stop=threading.Event();lock=threading.Lock();samples=[]
initial=call('/api/status')[0];run=f'bench-{int(time.time())}'
def reader(index):
    names=list(queries);i=index
    while not stop.is_set():
        name=names[i%len(names)];i+=1;start=time.perf_counter()
        try:
            _,ms=call(queries[name])
            with lock:latencies[name].append(ms)
        except Exception as e:
            with lock:errors.append(str(e))
        stop.wait(max(0,.1-(time.perf_counter()-start)))
def monitor():
    while not stop.is_set():
        try:
            value,_=call('/api/status');samples.append(value)
        except Exception as e:errors.append(str(e))
        stop.wait(.25)
def phase(rate,seconds,offset):
    count=0;start=time.perf_counter()
    for batch in range(seconds*10):
        target=start+batch*.1;time.sleep(max(0,target-time.perf_counter()))
        records=[]
        for j in range(rate//10):
            i=offset+count+j;stamp=datetime.datetime.now(datetime.timezone.utc).isoformat();records.append({'object':'trace.span','id':f'{run}-{i}','trace_id':f'{run}-trace-{i//20}','started_at':stamp,'ended_at':stamp,'span_data':{'type':'generation','model':'model-a','input':[{'role':'user','content':'x'*3500}],'output':[{'role':'assistant','content':'Measured output.'}]}})
        response,ms=call('/v1/traces/ingest','POST',json.dumps({'data':records}),True);write_latencies.append(ms)
        if response['accepted']!=len(records) or response['rejected']!=0:errors.append('Ingest count mismatch')
        count+=len(records)
    time.sleep(max(0,start+seconds-time.perf_counter()))
    elapsed=time.perf_counter()-start
    return {'targetSpansPerSecond':rate,'seconds':round(elapsed,3),'accepted':count,'achievedSpansPerSecond':round(count/elapsed,1)}
started=time.perf_counter()
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
    futures=[pool.submit(reader,i) for i in range(4)]+[pool.submit(monitor)]
    phases=[]
    try:
        phases.append(phase(100,args.seconds,0));phases.append(phase(1000,args.peak_seconds,phases[0]['accepted']))
        drain_start=time.perf_counter()
        while call('/api/status')[0]['pending']:
            if time.perf_counter()-drain_start>30:raise RuntimeError('Queue did not drain')
            time.sleep(.05)
        drain=time.perf_counter()-drain_start
    finally:stop.set()
    for f in futures:f.result()
final=call('/api/status')[0]
def summary(values):
    values=sorted(values)
    return {'count':len(values),'p50Ms':round(statistics.median(values),2),'p95Ms':round(values[min(len(values)-1,int(len(values)*.95))],2),'maxMs':round(max(values),2)} if values else {}
result={'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'host':platform.platform(),'cpu':platform.processor(),'seedSpans':creds['spans'],'seedPayloadBytes':creds['payloadBytes'],'readConcurrency':4,'readTargetRps':40,'phases':phases,'queries':{name:summary(values) for name,values in latencies.items()},'ingestion':summary(write_latencies),'drainSeconds':round(drain,3),'committedDuringRun':final['committed']-initial['committed'],'maxPending':max((s['pending'] for s in samples),default=0),'maxQueuedBytes':max((s['queuedBytes'] for s in samples),default=0),'errors':errors,'elapsedSeconds':round(time.perf_counter()-started,2)}
if args.database:
    import sqlite3
    with sqlite3.connect(f'file:{args.database}?mode=ro',uri=True) as conn:
        result['persistedNewSpans']=conn.execute('SELECT count(*) FROM spans WHERE id LIKE ?', (run+'-%',)).fetchone()[0]
    result['databaseBytes']=Path(args.database).stat().st_size
Path(args.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
expected=sum(p['accepted'] for p in phases)
assert not errors and result['committedDuringRun']==expected
if args.database: assert result['persistedNewSpans']==expected
assert all(v['p95Ms']<=200 for v in result['queries'].values()),'Read latency target missed; inspect report'

assert all(phase["achievedSpansPerSecond"] >= phase["targetSpansPerSecond"]*.95 for phase in phases), "Write rate target missed"
