---
name: mustard:dashboard
description: Inicia ou exibe URL do dashboard local (specs, métricas, criação de PRD). Use quando o usuário pedir para ver specs em uma página, abrir dashboard, criar PRD via UI.
---

# /mustard:dashboard - Local Dashboard

## Trigger
`/mustard:dashboard [start|stop|status]` (default: `start`)

## What it does
Inicia (ou exibe status de) um servidor HTTP local **com porta determinística por projeto** (range `7878–7977`, hash do `cwd`) que expõe quatro tabs — Visão, Specs, Métricas e Novo PRD — lendo specs de `.claude/spec/{active,completed}` e métricas de `.claude/.metrics/` e `.claude/.harness/events.jsonl` em tempo real. Tab "Novo PRD" gera spec.md em `.claude/spec/active/<data>-<slug>/`.

Cada projeto recebe sua própria porta — rodar simultaneamente em vários projetos não conflita.

## Action

### `start` (default)

```bash
node -e "const fs=require('fs'),path=require('path'),{spawn}=require('child_process'),http=require('http');const pidFile=path.join('.claude','.dashboard.pid'),portFile=path.join('.claude','.dashboard.port');function alive(p){try{process.kill(p,0);return true;}catch(_){return false;}}function probeInfo(port,cb){const r=http.request({host:'127.0.0.1',port,method:'GET',path:'/api/info',timeout:1500},res=>{if(res.statusCode!==200){res.resume();return cb(new Error('status'));}let b='';res.setEncoding('utf8');res.on('data',c=>b+=c);res.on('end',()=>{try{cb(null,JSON.parse(b));}catch(e){cb(e);}});});r.on('error',cb);r.on('timeout',()=>{r.destroy(new Error('timeout'));});r.end();}function readPort(){try{return parseInt(fs.readFileSync(portFile,'utf8'),10);}catch(_){return null;}}function done(port,info,prefix){console.log(prefix+' http://localhost:'+port+' (pid '+info.pid+', root '+info.root+')');process.exit(0);}const existingPort=readPort(),existingPid=fs.existsSync(pidFile)?parseInt(fs.readFileSync(pidFile,'utf8'),10):null;if(existingPort&&existingPid&&alive(existingPid)){probeInfo(existingPort,(err,info)=>{if(!err&&info&&info.root===process.cwd()){done(info.port,info,'Already running.');return;}console.log('Stale or mismatched dashboard state — spawning fresh.');spawnIt();});}else{if(existingPid&&!alive(existingPid)){try{fs.unlinkSync(pidFile);}catch(_){}try{fs.unlinkSync(portFile);}catch(_){}}spawnIt();}function spawnIt(){const child=spawn('node',['.claude/scripts/dashboard.js'],{detached:true,stdio:'ignore',windowsHide:true});child.unref();setTimeout(()=>{const p=readPort();if(!p){console.log('Spawn issued (pid '+child.pid+') but no port file yet — check .claude/scripts/dashboard.js logs.');process.exit(0);}probeInfo(p,(err,info)=>{if(err||!info){console.log('Spawn issued (pid '+child.pid+') but /api/info failed: '+(err&&err.message||'no body'));process.exit(0);}if(info.root!==process.cwd()){console.log('WARNING: port '+p+' bound to '+info.root+', not this project. Stop that dashboard first.');process.exit(1);}done(info.port,info,'Started.');});},1500);}"
```

### `stop`

```bash
node -e "const fs=require('fs'),path=require('path');const pidFile=path.join('.claude','.dashboard.pid'),portFile=path.join('.claude','.dashboard.port');if(!fs.existsSync(pidFile)){console.log('Not running.');process.exit(0);}const pid=parseInt(fs.readFileSync(pidFile,'utf8'),10);try{process.kill(pid);console.log('Stopped (pid '+pid+').');}catch(e){console.log('Process not found: '+e.message);}try{fs.unlinkSync(pidFile);}catch(_){}try{fs.unlinkSync(portFile);}catch(_){}"
```

### `status`

```bash
node -e "const fs=require('fs'),path=require('path'),http=require('http');const pidFile=path.join('.claude','.dashboard.pid'),portFile=path.join('.claude','.dashboard.port');if(!fs.existsSync(pidFile)){console.log('stopped');process.exit(0);}const pid=parseInt(fs.readFileSync(pidFile,'utf8'),10);try{process.kill(pid,0);}catch(_){console.log('stopped (stale pid file)');process.exit(0);}let port=null;try{port=parseInt(fs.readFileSync(portFile,'utf8'),10);}catch(_){}if(!port){console.log('running (pid '+pid+') — port unknown');process.exit(0);}const r=http.request({host:'127.0.0.1',port,method:'GET',path:'/api/info',timeout:1500},res=>{let b='';res.setEncoding('utf8');res.on('data',c=>b+=c);res.on('end',()=>{try{const info=JSON.parse(b);console.log('running (pid '+pid+') — http://localhost:'+port+' (root '+info.root+')');}catch(_){console.log('running (pid '+pid+') — http://localhost:'+port+' (probe parse failed)');}});});r.on('error',()=>console.log('running (pid '+pid+') — http://localhost:'+port+' (probe failed)'));r.on('timeout',()=>{r.destroy();console.log('running (pid '+pid+') — http://localhost:'+port+' (probe timeout)');});r.end();"
```

## Rules
- Porta = `7878 + (sha1(cwd) % 100)`. Em colisão (raro) ou se a porta hash já estiver bound para **outro** projeto, incrementa até achar livre.
- `GET /api/info` retorna `{ root, pid, branch, port }` — usado pelo start/status pra validar que a instância correta está rodando.
- All endpoints localhost-only; no auth, no CORS.
- POST /api/prd refuses overwrites (409 if `<date>-<slug>/` already exists).
- Logs to stdout when not detached; detached mode silences logs.
- `.claude/.dashboard.pid` e `.claude/.dashboard.port` são gitignored.
