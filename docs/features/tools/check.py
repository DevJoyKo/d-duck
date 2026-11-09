#!/usr/bin/env python3
"""유지보수 점검: (1) 연결된 자동 테스트가 아직 있는지 (2) OpenSpec에 catalog에 없는 요구사항·시나리오가 생겼는지.
Run from repo root: python3 docs/features/tools/check.py   (문제가 있으면 exit 1)"""
import json,os,re,glob,sys
H=os.path.dirname(os.path.dirname(os.path.abspath(__file__))); R=os.path.dirname(os.path.dirname(H)); APP=R+'/app'
C=json.load(open(f'{H}/catalog.json')); bad=[]
for f in C['features']:
    for s in f['scenarios']:
        for a in s['auto']:
            p,_,name=a.partition('::'); fp=f'{APP}/{p}'
            if not os.path.exists(fp): bad.append(f"{s['tc']}: 파일 없음 {p}"); continue
            src=open(fp,encoding='utf-8').read()
            if name and name not in src and name.replace('"','\\"') not in src and name.split('%')[0] not in src: bad.append(f"{s['tc']}: 테스트 없음 {a}")
known={(f['capability'],f['title']):f for f in C['features']}
for sp in sorted(glob.glob(R+'/openspec/changes/*/specs/*/spec.md')+glob.glob(R+'/openspec/specs/*/spec.md')):
    cap=sp.split('/')[-2]; req=None
    for line in open(sp,encoding='utf-8'):
        m=re.match(r'^### Requirement:\s*(.+)',line)
        if m:
            req=known.get((cap,m.group(1).strip()))
            if not req: bad.append(f'새 요구사항(ID 없음): {cap} / {m.group(1).strip()} — {os.path.relpath(sp,R)}')
            continue
        m=re.match(r'^#### Scenario:\s*(.+)',line)
        # 다른 change가 MODIFIED로 대체한 요구사항은 그 change의 시나리오만 본다
        if req and req.get('modified_by') and sp.split('/')[-4]!=req['modified_by']: continue
        if m and req and m.group(1).strip() not in {s['title'] for s in req['scenarios']}:
            bad.append(f"새 시나리오(TC 없음): {req['id']} / {m.group(1).strip()}")
print('\n'.join(bad) if bad else 'OK: 모든 자동 테스트 연결과 요구사항 ID가 맞아요')
sys.exit(1 if bad else 0)
