#!/usr/bin/env python3
"""catalog.json -> FEATURES.md, TEST-CASES.md, tc-mapping.csv. Run: python3 docs/features/tools/render.py"""
import json,csv,re,os,collections
H=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
C=json.load(open(f'{H}/catalog.json'))
F=C['features']; dom={d['code']:d for d in C['domains']}
def cell(s): return re.sub(r'\s+',' ',s.replace('|','\\|')).strip()
def wt(t):
    t=re.sub(r'\*\*(WHEN|THEN|AND|GIVEN)\*\*',r'\1',t)
    parts=[re.sub(r'^-\s*','',l).strip() for l in t.split('\n') if l.strip()]
    return ' / '.join(parts)
def spec_path(f):
    a=f"openspec/specs/{f['capability']}/spec.md"
    return a if os.path.exists(os.path.join(os.path.dirname(H),'..',a)) and not os.path.exists(os.path.join(os.path.dirname(H),'..',f"openspec/changes/{f['change']}")) else f"openspec/changes/{f['change']}/specs/{f['capability']}/spec.md"
def first(s,n=110):
    s=cell(s); return s if len(s)<=n else s[:n].rstrip()+'…'
ST=collections.Counter(f['status'] for f in F)
CV=collections.Counter(s['coverage'] for f in F for s in f['scenarios'])
ntc=sum(len(f['scenarios']) for f in F)
# FEATURES.md
o=['# D.Duck 기능 목록','',f'> 자동 생성: `docs/features/tools/render.py` · 원본 `catalog.json`. 손으로 고치지 말고 catalog.json을 고친 뒤 다시 만든다.','',
f'기능 {len(F)}개 · TC {ntc}개 · 상태: '+' · '.join(f'{k} {v}' for k,v in ST.most_common()),'',
'## 영역','','| 코드 | 영역 | 기능 | TC | OpenSpec capability |','|---|---|---|---|---|']
for d in C['domains']:
    fs=[f for f in F if f['domain']==d['code']]
    o.append(f"| {d['code']} | [{d['name']}](#{d['code'].lower()}) | {len(fs)} | {sum(len(f['scenarios']) for f in fs)} | {', '.join(d['capabilities'])} |")
for d in C['domains']:
    o+=['',f"## {d['code']}",'',f"**{d['name']}**",'','| ID | 기능 | 상태 | 요약 | 근거 change | 핵심 코드 | TC |','|---|---|---|---|---|---|---|']
    for f in [f for f in F if f['domain']==d['code']]:
        a=sum(s['coverage']=='자동' for s in f['scenarios'])
        o.append(f"| {f['id']} | {cell(f['title'])} | {f['status']} | {first(f['body'])} | {f['change']}{' → '+f['modified_by'] if f.get('modified_by') else ''} | {'<br>'.join('`'+c+'`' for c in f.get('code',[]))} | [{len(f['scenarios'])}개 · 자동 {a}](TEST-CASES.md#{f['id'].lower()}) |")
open(f'{H}/FEATURES.md','w').write('\n'.join(o)+'\n')
# TEST-CASES.md
o=['# D.Duck 테스트 케이스(TC) 매핑','',f'> 자동 생성: `docs/features/tools/render.py`. TC ID = `TC-<영역>-<기능번호>-<시나리오번호>`, 시나리오는 OpenSpec spec의 `#### Scenario`와 1:1이다.','',
f'TC {ntc}개 · '+' · '.join(f'{k} {v}' for k,v in CV.most_common()),'',
'- **자동**: 연결된 테스트가 시나리오의 조건과 기대 결과를 확인한다. 회귀는 `cargo test -p dduck-core`, `npx vitest run`(app/)으로 돌린다.',
'- **부분 자동**: 일부만 자동이다. 나머지는 수동 절차로 확인한다.',
'- **수동**: 앱(또는 `npm run dev:mock`)에서 절차대로 확인한다. `[스파이크]`는 실제 한도·로그인처럼 재현 비용이 큰 확인이다.','']
for d in C['domains']:
    o+=['',f"## {d['code']} · {d['name']}"]
    for f in [f for f in F if f['domain']==d['code']]:
        o+=['',f"### {f['id']}",'',f"**{cell(f['title'])}** · {f['status']}{' — '+cell(f['status_note']) if f.get('status_note') else ''} · spec: `{spec_path(f)}`",'',
            '| TC | 시나리오 | 조건 → 기대 결과 | 방식 | 자동 테스트 / 수동 절차 |','|---|---|---|---|---|']
        for s in f['scenarios']:
            how='<br>'.join('`'+a+'`' for a in s['auto'])
            if s['manual']: how=(how+'<br>' if how else '')+'수동: '+cell(s['manual'])
            o.append(f"| {s['tc']} | {cell(s['title'])} | {cell(wt(s['text']))} | {s['coverage']} | {how or '-'} |")
        if not f['scenarios']: o.append('| - | (시나리오 없음) | - | - | - |')
open(f'{H}/TEST-CASES.md','w').write('\n'.join(o)+'\n')
# CSV (spreadsheet / QA 실행 기록용)
with open(f'{H}/tc-mapping.csv','w',newline='',encoding='utf-8-sig') as fp:
    w=csv.writer(fp); w.writerow(['TC ID','Feature ID','영역','기능','기능 상태','시나리오','조건 → 기대 결과','방식','자동 테스트','수동 절차','change','spec 경로','실행 결과','실행일','비고'])
    for f in F:
        for s in f['scenarios']:
            w.writerow([s['tc'],f['id'],dom[f['domain']]['name'],f['title'],f['status'],s['title'],wt(s['text']),s['coverage'],'\n'.join(s['auto']),s['manual'],f['change'],spec_path(f),'','',''])
print(f'{len(F)} features, {ntc} TCs', dict(ST), dict(CV))
