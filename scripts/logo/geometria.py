import math, sys, json
def build(g=0.075, w=0.24, b=0.24, c=0.2, s=100.0):
    k=math.sqrt(3)/2
    UL=(-k*s,-0.5*s); UR=(k*s,-0.5*s); D=(0,s)
    P0=(0.0,0.0)
    def top(i,j): return (P0[0]+i*UL[0]+j*UR[0], P0[1]+i*UL[1]+j*UR[1])
    def left(u,v): return (u*UL[0]+v*D[0], u*UL[1]+v*D[1])
    def right(u,v): return (u*UR[0]+v*D[0], u*UR[1]+v*D[1])
    h=g/2
    pieces={}
    # Cara izquierda/derecha
    for name,f in (('Left',left),('Right',right)):
        outer=[(1,h),(1,1),(h,1),(h,1-w),(1-w,1-w),(1-w,h)]
        inner=[(h,h),(1-w-g,h),(1-w-g,h+w),(h+w,h+w),(h+w,1-w-g),(h,1-w-g)]
        pieces['outer'+name]=[f(*p) for p in outer]
        pieces['inner'+name]=[f(*p) for p in inner]
    cz=h+c+g
    pieces['core']=[top(*p) for p in [(h,h),(h+c,h),(h+c,h+c),(h,h+c)]]
    ring=[(cz,h),(1,h),(1,1),(h,1),(h,cz),(h+b,cz),(h+b,h+b) if False else (h+b,cz)]
    ring=[(cz,h),(1,h),(1,1),(h,1),(h,cz),(h+b,cz),(h+b,1-b),(1-b,1-b),(1-b,h+b),(cz,h+b)]
    pieces['top']=[top(*p) for p in ring]
    return pieces
def path(poly):
    return "M"+" L".join(f"{x:.2f} {y:.2f}" for x,y in poly)+" Z"
def svg(p, size=512, bg="#1a2a6a", fg="#e8ecf8", core="#6c9bff"):
    out=[]
    for k,v in p.items():
        out.append(f'<path d="{path(v)}" fill="{core if k=="core" else fg}"/>')
    k=math.sqrt(3)/2*100
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{-k-4:.1f} -104 {2*k+8:.1f} 208" width="{size}" height="{size}"><rect x="-200" y="-200" width="400" height="400" fill="{bg}"/>'+"".join(out)+'</svg>'
if __name__=="__main__":
    args=json.loads(sys.argv[1]) if len(sys.argv)>1 else {}
    open('/tmp/logo/t.svg','w').write(svg(build(**args)))
