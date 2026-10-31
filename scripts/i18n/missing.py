# Lista los textos extraídos que todavía no están en app/src/i18n/en.json.
# Uso: python3 scripts/i18n/missing.py i18n-ui.json i18n-rust.json
import json, re, sys, os
en = json.load(open(os.path.join(os.path.dirname(__file__), "..", "..", "app", "src", "i18n", "en.json")))
have = set(en["exact"]) | {k for k, _ in en["patterns"]}
for f in sys.argv[1:]:
    d = json.load(open(f))
    for e in d["exact"] + d["patterns"]:
        k = e["k"]
        if k in have or k.strip() in have:
            continue
        print(json.dumps({"k": k, "f": e["f"]}, ensure_ascii=False))
