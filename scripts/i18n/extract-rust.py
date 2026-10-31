import re, os, json
root=os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','..','crates')
exact={}; pats={}
strre=re.compile(r'(?<![A-Za-z0-9_])(r#?)?"((?:[^"\\]|\\.)*)"#?')
for dp,dn,fn in os.walk(root):
    if '/tests' in dp: continue
    for f in fn:
        if not f.endswith('.rs'): continue
        p=os.path.join(dp,f)
        src=open(p,encoding='utf-8').read()
        # cortar módulo de pruebas
        i=src.find('#[cfg(test)]')
        if i>=0: src=src[:i]
        for line in src.split('\n'):
            s=line.strip()
            if s.startswith('//') or s.startswith('#[') or s.startswith('///'): continue
            for m in strre.finditer(line):
                t=m.group(2)
                if not re.search(r'[a-záéíóúñ]{3,}',t,re.I): continue
                if not re.search(r'[áéíóúñ¿¡]|\b(el|la|los|las|de|del|que|no|se|un|una|para|con|por|ya|es|tu|tus|hay|más)\b',t,re.I): continue
                t=t.replace('\\"','"').replace('\\n','\n')
                if '{' in t and re.search(r'\{[^}]*\}',t):
                    k=re.sub(r'\{[^{}]*\}','{}',t)
                    n=[0]
                    def rep(_):
                        n[0]+=1; return '{%d}'%(n[0]-1)
                    k=re.sub(r'\{\}',rep,k)
                    pats.setdefault(k,set()).add(os.path.relpath(p,root))
                else:
                    exact.setdefault(t,set()).add(os.path.relpath(p,root))
out={'exact':[{'k':k,'f':sorted(v)} for k,v in exact.items()],'patterns':[{'k':k,'f':sorted(v)} for k,v in pats.items()]}
import sys
json.dump(out,open(sys.argv[1] if len(sys.argv)>1 else 'i18n-rust.json','w'),ensure_ascii=False,indent=1)
print(len(exact),len(pats),sum(len(k) for k in exact)+sum(len(k) for k in pats))
