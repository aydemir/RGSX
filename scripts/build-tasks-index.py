#!/usr/bin/env python3
"""
tasks/index.json generator — front-matter + legacy bullet her ikisini de parse eder.
AGENTS.md Task Sorgu Protokolü'nün tek kaynağı.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TASKS_DIR = ROOT / "tasks"
OUTPUT = TASKS_DIR / "index.json"

# YAML front-matter için PyYAML yoksa fallback regex kullan
try:
    import yaml
    HAS_YAML = True
except ImportError:
    HAS_YAML = False

STATUS_CANON = {
    "todo": "todo",
    "in_progress": "in_progress",
    "in-progress": "in_progress",
    "done": "done",
    "superseded": "superseded",
    "documented-only": "documented-only",
    "documented_only": "documented-only",
}
PRIORITY_CANON = {
    "P0": "P0", "p0": "P0", "high": "P0",
    "P1": "P1", "p1": "P1", "medium": "P1",
    "P2": "P2", "p2": "P2", "low": "P2",
    "P3": "P3",
}

def canon_status(s):
    if not s: return "todo"
    s = str(s).strip()
    low = s.lower()
    # direct map first
    if s in STATUS_CANON:
        return STATUS_CANON[s]
    if low in STATUS_CANON:
        return STATUS_CANON[low]
    # prefix match for legacy free-form statuses
    if low.startswith("done") or low.startswith("completed") or low.startswith("closed"):
        return "done"
    if low.startswith("todo"):
        return "todo"
    if low.startswith("in_progress") or low.startswith("in-progress") or low.startswith("in progress"):
        return "in_progress"
    if low.startswith("superseded"):
        return "superseded"
    if low.startswith("implemented"):
        return "done"
    return STATUS_CANON.get(s, s)

def canon_priority(s):
    if not s: return "P2"
    s = str(s).strip()
    # handle "P0 (high)" etc — take first token
    tok = s.split()[0].split("(")[0].strip()
    if tok in PRIORITY_CANON:
        return PRIORITY_CANON[tok]
    if tok.lower() in PRIORITY_CANON:
        return PRIORITY_CANON[tok.lower()]
    return PRIORITY_CANON.get(s, s)

def parse_front_matter(text):
    """--- YAML front-matter varsa dict döndür, yoksa None."""
    if not text.startswith("---"):
        return None, text
    # ikinci --- bul
    m = re.match(r'^---\s*\n(.*?)\n---\s*\n', text, re.DOTALL)
    if not m:
        return None, text
    fm_text = m.group(1)
    rest = text[m.end():]
    if HAS_YAML:
        try:
            data = yaml.safe_load(fm_text) or {}
            return data, rest
        except Exception as e:
            print(f"WARN: YAML parse error: {e}\n{fm_text[:300]}", file=sys.stderr)
            return {}, rest
    # fallback: basit key: value parse
    data = {}
    for line in fm_text.splitlines():
        line=line.strip()
        if not line or line.startswith("#"): continue
        if ":" not in line: continue
        k,v = line.split(":",1)
        k=k.strip(); v=v.strip().strip('"').strip("'")
        # list parse: [a, b]
        if v.startswith("["):
            try:
                v = json.loads(v.replace("'",'"'))
            except:
                v = [x.strip().strip('"').strip("'") for x in v.strip("[]").split(",") if x.strip()]
        data[k]=v
    return data, rest

def parse_legacy_bullets(text):
    """- **key:** value bullet'larını dict'e çevir."""
    data = {}
    # pattern: - **key:** value  veya - **key**: value
    pat = re.compile(r'^\s*-\s+\*\*(\w+):\*\*\s*(.+)$', re.MULTILINE)
    # alternatif: - **key:** value  (colon inside bold)
    pat2 = re.compile(r'^\s*-\s+\*\*(\w+)\*\*:\s*(.+)$', re.MULTILINE)
    for m in pat.finditer(text):
        k=m.group(1).strip(); v=m.group(2).strip()
        data[k]=v
    for m in pat2.finditer(text):
        k=m.group(1).strip(); v=m.group(2).strip()
        if k not in data:
            data[k]=v
    # tags / depends_on / labels özel
    for k in list(data.keys()):
        v=data[k]
        if k in ("tags","labels","depends_on","dependsOn"):
            # [a, b] veya a, b
            if isinstance(v, str):
                v=v.strip()
                if v.startswith("["):
                    try:
                        v=json.loads(v.replace("'",'"'))
                    except:
                        v=[x.strip().strip('"').strip("'") for x in v.strip("[]").split(",") if x.strip()]
                elif "," in v:
                    v=[x.strip() for x in v.split(",") if x.strip()]
                else:
                    v=[v] if v else []
            data[k]=v
    return data

def normalize_entry(path: Path, fm: dict, legacy: dict):
    # front-matter öncelikli, legacy fallback
    merged = {**legacy, **fm}
    # key normalizasyon
    # id
    id_val = merged.get("id") or merged.get("ID")
    if not id_val:
        # dosyadan çıkar: TASK-XXX
        m = re.search(r'(TASK-[0-9A-Za-z\-]+)', path.name)
        id_val = m.group(1) if m else path.stem
    # title
    title = merged.get("title") or merged.get("Title") or ""
    # status
    status = canon_status(merged.get("status") or merged.get("Status") or "todo")
    # klasörden status çıkarımı (legacy dosyalar klasörle uyumlu olmalı ama index status'ü esas)
    # done klasöründekiler done say
    if "done" in path.parts and status == "todo":
        # header todo ama done klasöründeyse done kabul et (bayat header)
        # ama explicit done/in_progress korunmalı
        pass
    # priority
    priority = canon_priority(merged.get("priority") or merged.get("Priority") or "P2")
    # environment
    env = merged.get("environment") or merged.get("Environment") or "both"
    env = str(env).strip().lower()
    if env not in ("linux","windows","both"):
        env = "both"
    # updated / created
    updated = merged.get("updated") or merged.get("Updated") or merged.get("created") or merged.get("Created") or ""
    # labels / tags birleştir
    labels = merged.get("labels") or merged.get("Labels") or merged.get("tags") or merged.get("Tags") or []
    if isinstance(labels, str):
        labels = [labels]
    tags = merged.get("tags") or merged.get("Tags") or []
    if isinstance(tags, str):
        tags = [tags]
    # labels ve tags merge (labels tercih)
    if labels and tags and labels != tags:
        # birleştir
        combined = list(dict.fromkeys(list(labels)+list(tags)))
    elif labels:
        combined = labels
    else:
        combined = tags
    # depends_on
    depends_on = merged.get("depends_on") or merged.get("dependsOn") or merged.get("Depends_on") or []
    if isinstance(depends_on, str):
        depends_on = [depends_on] if depends_on else []
    # file relative
    rel = str(path.relative_to(ROOT))
    # superseded_by
    superseded_by = merged.get("superseded_by") or merged.get("supersededBy") or merged.get("superseded") or ""
    return {
        "id": str(id_val).strip(),
        "title": str(title).strip().strip('"').strip("'"),
        "status": status,
        "priority": priority,
        "environment": env,
        "labels": combined,
        "depends_on": depends_on,
        "updated": str(updated).strip(),
        "file": rel,
        "superseded_by": str(superseded_by).strip(),
    }

def main():
    if not TASKS_DIR.exists():
        print(f"tasks dir not found: {TASKS_DIR}", file=sys.stderr)
        sys.exit(1)
    entries = []
    seen_ids = {}
    for md in sorted(TASKS_DIR.rglob("*.md")):
        if md.name == "index.json": continue
        if md.name.startswith("_"): continue
        if "KANBAN" in md.name: continue
        text = md.read_text(encoding="utf-8", errors="replace")
        fm, rest = parse_front_matter(text)
        if fm is not None:
            legacy = parse_legacy_bullets(rest)
            # front-matter var, legacy'yi rest'ten al
        else:
            fm = {}
            legacy = parse_legacy_bullets(text)
        # id yoksa atla (örn gap/Değiştirilen TASK Maddeleri)
        entry = normalize_entry(md, fm, legacy)
        # filtre: id TASK- ile başlamıyorsa skip (dokümantasyon dosyaları)
        if not entry["id"].startswith("TASK-"):
            continue
        # duplicate id uyar
        if entry["id"] in seen_ids:
            print(f"WARN: duplicate id {entry['id']}: {seen_ids[entry['id']]} vs {entry['file']}", file=sys.stderr)
        seen_ids[entry["id"]] = entry["file"]
        entries.append(entry)

    # sıralama: id
    entries.sort(key=lambda x: x["id"])

    # yaz
    OUTPUT.write_text(json.dumps(entries, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Wrote {len(entries)} entries -> {OUTPUT.relative_to(ROOT)}")
    # kısa istatistik
    from collections import Counter
    c = Counter(e["status"] for e in entries)
    print(f"status breakdown: {dict(c)}")

if __name__ == "__main__":
    main()
