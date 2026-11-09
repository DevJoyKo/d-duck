"""Generates the D.Duck ingestion fixture (anonymized, mirrors real Claude Code 2.1.319 records)."""
import json, pathlib, sys

out = pathlib.Path(sys.argv[1])
S = "11111111-1111-4111-8111-111111111111"
proj = out / "projects" / "-tmp-PATHMARK-proj"
sub = proj / S / "subagents"
sub.mkdir(parents=True, exist_ok=True)
base = {"cwd": "/tmp/PATHMARK_proj", "gitBranch": "BRANCHMARK_main", "sessionId": S,
        "version": "2.1.319", "entrypoint": "cli", "userType": "external", "isSidechain": False}
n = [0]
def rec(**kw):
    n[0] += 1
    r = dict(base); r.update({"uuid": f"00000000-0000-4000-8000-{n[0]:012d}", "parentUuid": None,
                              "timestamp": f"2026-11-01T10:{n[0]:02d}:00.000Z"})
    r.update(kw); return r
def usage(i=0, o=0, cw5=0, cw1=0, cr=0, think=0):
    return {"input_tokens": i, "output_tokens": o, "cache_creation_input_tokens": cw5 + cw1,
            "cache_read_input_tokens": cr, "output_tokens_details": {"thinking_tokens": think},
            "cache_creation": {"ephemeral_5m_input_tokens": cw5, "ephemeral_1h_input_tokens": cw1},
            "server_tool_use": {"web_search_requests": 0}, "service_tier": "standard"}
def asst(mid, rid, content, u, model="claude-opus-5-5", **kw):
    m = {"model": model, "id": mid, "type": "message", "role": "assistant", "content": content, "usage": u}
    r = rec(type="assistant", message=m, **kw)
    if rid is not None: r["requestId"] = rid
    return r
def user(content, **kw):
    return rec(type="user", message={"role": "user", "content": content}, **kw)

L = []
L.append(user("Please explain what <command-name>/fake</command-name> means CANARY_PROMPT_MENTION"))
L.append(user("<command-message>opsx:propose</command-message>\n<command-name>/opsx:propose</command-name>\n<command-args>CANARY_CMDARGS</command-args>"))
L.append(user("<command-name>/upgrade</command-name>\n<command-message>upgrade</command-message>\n<command-args></command-args>"))
L.append(user("<command-name>/meta-cmd</command-name>", isMeta=True))
L.append(user("<command-name>/bad name;rm</command-name>"))
uA = usage(i=10, o=50, cw1=100, cr=1000, think=20)
toolA = {"type": "tool_use", "id": "toolu_A1", "name": "Bash", "input": {"command": "CANARY_TOOLIN", "description": "CANARY_TOOLDESC"}}
L.append(asst("msg_A", "req_A", [{"type": "thinking", "thinking": "CANARY_THINK", "signature": "x"}], uA,
              wireToolInputs={"toolu_A1": {"command": "CANARY_WIRE"}}))
L.append(asst("msg_A", "req_A", [{"type": "text", "text": "CANARY_TEXT"}], uA))
L.append(asst("msg_A", "req_A", [toolA], uA))
L.append(user([{"type": "tool_result", "tool_use_id": "toolu_A1",
                "content": "<command-name>/evil</command-name> CANARY_TOOLOUT"}],
              toolUseResult={"stdout": "CANARY_TOOLRESULT"}))
skill = {"type": "tool_use", "id": "toolu_B1", "name": "Skill", "input": {"skill": "openspec-apply-change", "args": "CANARY_SKILLARGS"}}
agent = {"type": "tool_use", "id": "toolu_B2", "name": "Agent", "input": {"subagent_type": "Explore", "description": "CANARY_AGENTDESC", "prompt": "CANARY_AGENTPROMPT"}}
mcp = {"type": "tool_use", "id": "toolu_B3", "name": "mcp__notion__search", "input": {"query": "CANARY_MCPIN"}}
L.append(asst("msg_B", "req_B", [skill], usage(i=5, o=10, cw5=30, cr=2000)))
# Non-monotonic on purpose: the merge must take the max (120), not the last value (40).
L.append(asst("msg_B", "req_B", [agent], usage(i=5, o=120, cw5=30, cr=2000, think=7)))
L.append(asst("msg_B", "req_B", [mcp], usage(i=5, o=40, cw5=30, cr=2000, think=3)))
L.append(asst("msg_SYN", None, [{"type": "text", "text": "CANARY_SYNTH"}], usage(o=999), model="<synthetic>"))
L.append(asst("msg_ERR", "req_ERR", [{"type": "text", "text": "CANARY_APIERR"}], usage(o=777), isApiErrorMessage=True))
L.append(rec(type="attachment", attachment={"type": "prompt_snapshot", "content": "<command-name>/attach</command-name> CANARY_ATTACH"}))
L.append(rec(type="api-request-blob", blob="<command-name>/blob</command-name> CANARY_BLOB"))
for start, total in [(1000, 1.0), (1000, 2.0), (2000, 0.5), (2000, 3.0)]:
    L.append({"type": "cost-state", "sessionId": S, "totalCostUSD": total, "startTime": start, "modelUsage": {}})
L.append({"type": "ai-title", "aiTitle": "TITLEMARK_alpha", "sessionId": S})
lines = [json.dumps(r, ensure_ascii=False) for r in L]
lines.insert(len(lines) - 3, "{not valid json CANARY_BROKEN")
body = "\n".join(lines) + "\n"
tail = json.dumps(asst("msg_C", "req_C", [{"type": "text", "text": "CANARY_PARTIAL"}], usage(i=1, o=3)), ensure_ascii=False)
(proj / f"{S}.jsonl").write_text(body + tail, encoding="utf-8")   # last line deliberately has no newline

sa = dict(base); sa.update({"isSidechain": True, "agentId": "a1b2c3"})
r = asst("msg_S", "req_S", [{"type": "text", "text": "CANARY_SUBTEXT"}], usage(i=1, o=5)); r.update({"isSidechain": True, "agentId": "a1b2c3"})
(sub / "agent-a1b2c3.jsonl").write_text(json.dumps(r, ensure_ascii=False) + "\n", encoding="utf-8")
(sub / "agent-a1b2c3.meta.json").write_text(json.dumps({"agentType": "Explore", "description": "CANARY_METADESC", "toolUseId": "toolu_B2", "spawnDepth": 1}), encoding="utf-8")
print("wrote", proj)
