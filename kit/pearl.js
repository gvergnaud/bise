// bise ambient · the pearl: bise's body in the capsule (SPEC §1, round 8; replaces the
// constellation). A ball of pink mist with no rim, a cream light inside, lilac in its shadows, its
// edge thinning into the air and breaking into dots of light. The fragment shader is the
// reference's as is (site/content/ambient/marble.js, variant 1 'the mist pearl', palette 'pink').
//
//   const pearl = AmbPearl.create(canvas)
//   pearl.frame({ mood, lvl, still }, nowMs)   // returns whether it is still moving
//
// One WebGL1 context on the capsule's own canvas, one quad. Every mood value eases toward its
// target with 1 - e^(-dt/0.3) (a change takes ~0.9 s); the smoke's flow integrates
// dt · speed · (1 + 1.2 · voice · level), level being the real mic or TTS level. It draws at most
// 30 times a second; under reduced motion (`still`) the pearl stands still in every mood (its tint
// still eases), and once settled it draws nothing more.
(function (root) {
  "use strict";

  const FS = `
  precision highp float;
  uniform vec2 res; uniform float t, flow, variant, lvl, think, need, done, away, listen, still, gather, zoom;
  float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
  float vn(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);
    return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x),f.y);}
  float fbm(vec2 p){float s=0.,a=.5;for(int i=0;i<5;i++){s+=a*vn(p);p=mat2(1.6,1.2,-1.2,1.6)*p;a*=.5;}return s;}
  mat2 rot(float a){float c=cos(a),s=sin(a);return mat2(c,-s,s,c);}
  uniform vec3 UMBER, AMBER, CREAM, ROSE, OLIVE, EMBER;
  // the smoke inside: domain-warped noise, swirled into a whirlpool while bise thinks
  vec3 smoke(vec2 uv, float R, out float dens, out float z){
    float r=length(uv); z=sqrt(max(0.,1.-r*r/(R*R)));
    vec2 p=uv*1.5/(.55+z*.6);
    p=rot(think*(2.6*(1.-r/R))+flow*.12*think)*p;
    vec2 q=vec2(fbm(p+flow*.22),fbm(p+vec2(5.2,1.3)-flow*.18));
    vec2 w=vec2(fbm(p+3.*q+vec2(1.7,9.2)+flow*.3),fbm(p+3.*q+vec2(8.3,2.8)));
    float f=fbm(p+2.5*w); dens=f;
    vec3 col=mix(UMBER,AMBER,clamp(f*f*2.3,0.,1.));
    col=mix(col,CREAM,clamp(w.x*w.x*1.3,0.,1.)*.7);
    col=mix(col,ROSE,clamp(length(q)*.5,0.,1.)*.22);
    col+=CREAM*exp(-r*r/(R*R)*3.)*.35*(1.+listen*.5+lvl*.5);
    col=mix(col,col*.55+ROSE*.6*(.6+.4*f),need*.65);
    col=mix(col,col*.6+OLIVE*.55*(.6+.4*f),done*.6);
    col=mix(col,col*.5+EMBER*.35,away*.6);
    col*=(1.-away*.45);
    return col;}
  void main(){
    // zoom: the pearl's size inside its canvas (1 fills it); the bar grows it while fn is held here,
    // never with a CSS transform on the canvas (WebKit and Chrome showed a scaled WebGL canvas as a
    // thin vertical sliver, round 9's gate)
    vec2 uv=(gl_FragCoord.xy-.5*res)/min(res.x,res.y)*2./max(zoom,.01);
    // only well-defined GLSL below (WebKit's Metal path draws garbage where Chrome forgives:
    // ambient's gate, the listen pearl as a streak): no atan(0,0), no pow of a negative base, no
    // smoothstep with edge0 >= edge1 (written as 1 - smoothstep(lo, hi, x))
    float r=length(uv),an=atan(uv.y,uv.x+1e-6);
    float breath=.015*sin(t*6.283/7.)*(1.-still);
    float R=.66+breath+lvl*.05*sin(an*5.+t*7.)+lvl*.025*sin(an*9.-t*5.)+need*.025*sin(t*5.)+listen*.03;
    float halo=(.16+.35*lvl+.15*listen+.22*need+.12*done)*(1.-away*.6);
    // mist pearl: no rim. the smoke thins out into the air, and the edge breaks into dots of light
    R*=1.05; float dens,z; vec3 col=smoke(uv*.95,R,dens,z);
    float edge=R+.18*(fbm(vec2(an*2.,flow*.3))-.4);
    float a=(1.-smoothstep(edge-.45,edge,r))*(.55+.6*dens);
    vec2 g=uv*min(res.x,res.y)/6.; vec2 id=floor(g), fc=fract(g)-.5;
    float h=hash(id), tw=.5+.5*sin(t*(1.5+h*3.)+h*40.);
    float rd=(r-R*.98)/.16; float ring=exp(-rd*rd);
    float dot_=step(.82-ring*.25,h)*(1.-smoothstep(.0,.32,length(fc)))*ring*tw;
    col=mix(col,CREAM*1.1,dot_); a=max(a,dot_*.9);
    a+= (1.-a)*exp(-max(0.,r-R*.8)*5.)*halo*.6;
    // the hello (ambient m_5702): gather 1 is a few scattered dots of pink and cream, the body not
    // yet there; as it eases to 0 the dots close in and the mist fills in behind them
    if(gather>0.){
      vec2 gs=uv/(1.+gather*1.3); vec2 g2=gs*min(res.x,res.y)/7.; vec2 id2=floor(g2), f2=fract(g2)-.5;
      float h2=hash(id2+vec2(7.,3.));
      float d2=step(.8,h2)*(1.-smoothstep(.0,.3,length(f2)))*(1.-smoothstep(R*.6,R*1.05,length(gs)))*gather;
      a*=(1.-gather); col=mix(col,mix(ROSE,CREAM,fract(h2*13.)),d2); a=max(a,d2*.95);}
    gl_FragColor=vec4(col*a,a);}`;
  const VS = "attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}";

  // bise pink (SPEC §1): dark, body, light, needs you, done, away
  const PINK = { UMBER: [0.14, 0.05, 0.11], AMBER: [0.96, 0.56, 0.64], CREAM: [1, 0.93, 0.91], ROSE: [1, 0.38, 0.6], OLIVE: [1, 0.87, 0.66], EMBER: [0.58, 0.46, 0.96] };
  // moods: what each asks of the smoke (SPEC §1's table)
  const MOODS = {
    rest:   { speed: 0.18, think: 0, need: 0, done: 0, away: 0, listen: 0, voice: 0, still: 0 },
    still:  { speed: 0, think: 0, need: 0, done: 0, away: 0.25, listen: 0, voice: 0, still: 1 },
    listen: { speed: 0.7, think: 0, need: 0, done: 0, away: 0, listen: 1, voice: 1, still: 0 },
    speak:  { speed: 0.5, think: 0, need: 0, done: 0, away: 0, listen: 0.4, voice: 0.6, still: 0 },
    work:   { speed: 0.45, think: 1, need: 0, done: 0, away: 0, listen: 0, voice: 0, still: 0 },
    need:   { speed: 0.2, think: 0, need: 1, done: 0, away: 0, listen: 0, voice: 0, still: 0 },
    done:   { speed: 0.25, think: 0, need: 0, done: 1, away: 0, listen: 0, voice: 0, still: 0 },
    fail:   { speed: 0, think: 0, need: 0, done: 0, away: 0.5, listen: 0, voice: 0, still: 1 },
    away:   { speed: 0.06, think: 0, need: 0, done: 0, away: 1, listen: 0, voice: 0, still: 0 },
  };
  const KEYS = Object.keys(MOODS.rest);

  // one step of the mood state (pure, for the checks): ease toward the mood's targets, integrate
  // the flow; under reduced motion the pearl stands still (speed 0, no breath) whatever the mood
  function ease(S, mood, level, dt, reduce) {
    const M = { ...(MOODS[mood] || MOODS.rest) };
    if (reduce) { M.speed = 0; M.still = 1; M.voice = 0; }
    const k = dt > 1 ? 1 : 1 - Math.exp(-dt / 0.3);
    const N = { ...S };
    for (const key of KEYS) N[key] = S[key] + (M[key] - S[key]) * k;
    N.flow = S.flow + dt * N.speed * (1 + 1.2 * N.voice * (level || 0));
    return N;
  }
  // settled: every value at its target (nothing left to draw under reduced motion)
  function settled(S, mood, reduce) {
    const M = { ...(MOODS[mood] || MOODS.rest) };
    if (reduce) { M.speed = 0; M.still = 1; M.voice = 0; }
    return KEYS.every((key) => Math.abs(S[key] - M[key]) < 0.002);
  }
  const start = (flow) => ({ flow: flow || 0, ...MOODS.rest });

  // the shader on a canvas: draw(S, t, level) with the mood values S as they are (the capsule
  // eases them)
  function renderer(cv, opts) {
    const gl = cv.getContext("webgl", { premultipliedAlpha: true, alpha: true, antialias: false, ...(opts || {}) });
    if (!gl) return null;
    const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) console.error(gl.getShaderInfoLog(s)); return s; };
    const prog = gl.createProgram();
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, VS)); gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog); gl.useProgram(prog);
    const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p"); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const U = {};
    for (const k of ["res", "t", "flow", "variant", "lvl", "think", "need", "done", "away", "listen", "still", "gather", "zoom", ...Object.keys(PINK)]) U[k] = gl.getUniformLocation(prog, k);
    for (const k in PINK) gl.uniform3fv(U[k], PINK[k]);
    gl.uniform1f(U.variant, 1); gl.uniform1f(U.zoom, 1);
    function draw(S, t, level) {
      gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
      gl.uniform2f(U.res, gl.drawingBufferWidth, gl.drawingBufferHeight); gl.uniform1f(U.t, t); gl.uniform1f(U.flow, S.flow);
      gl.uniform1f(U.lvl, S.voice * (level || 0)); gl.uniform1f(U.think, S.think); gl.uniform1f(U.need, S.need);
      gl.uniform1f(U.done, S.done); gl.uniform1f(U.away, S.away); gl.uniform1f(U.listen, S.listen); gl.uniform1f(U.still, S.still);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
    return { draw, gl };
  }

  function create(cv) {
    const gl = cv.getContext("webgl", { premultipliedAlpha: true, alpha: true, antialias: false });
    let S = start(Math.random() * 40), last = null, drawnAt = -1e9, settledKey = "";
    if (!gl) return { frame: () => false, state: () => S, failed: true };
    const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) console.error(gl.getShaderInfoLog(s)); return s; };
    const prog = gl.createProgram();
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, VS)); gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog); gl.useProgram(prog);
    const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p"); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const U = {};
    for (const k of ["res", "t", "flow", "variant", "lvl", "think", "need", "done", "away", "listen", "still", "gather", "zoom", ...Object.keys(PINK)]) U[k] = gl.getUniformLocation(prog, k);
    for (const k in PINK) gl.uniform3fv(U[k], PINK[k]);
    gl.uniform1f(U.variant, 1); gl.uniform1f(U.zoom, 1);

    function fit() {
      const dpr = Math.min(2, root.devicePixelRatio || 1);
      // the pearl is round: one square buffer from the canvas's smaller side (a wrong width read
      // during the hold drew the pearl into a wide buffer that the 64 px box then squeezed into a
      // thin vertical streak: ambient's round 9 gate, WebKit and Chrome alike)
      const cw = cv.clientWidth, chh = cv.clientHeight;
      const side = Math.round(Math.min(cw || chh || 68, chh || cw || 68) * dpr);
      const w = side, h = side;
      if (cv.width !== w || cv.height !== h) { cv.width = w; cv.height = h; return true; }
      return false;
    }

    // one frame; returns whether the pearl is still moving (the loop keeps a 30 Hz pace while it is)
    function frame(st, nowMs) {
      const t = nowMs / 1000;
      const dt = last == null ? 1 : Math.max(0, t - last);
      last = t;
      S = ease(S, st.mood, st.lvl, dt, !!st.still);
      const resized = fit();
      const calm = st.still && settled(S, st.mood, true) && !(st.gather > 0);
      const key = calm ? `${st.mood}` : "";
      if (calm && !resized && key === settledKey) return false;
      settledKey = key;
      // at most 30 draws a second
      if (!calm && !resized && nowMs - drawnAt < 30) return true;
      drawnAt = nowMs;
      // the buffer the context really has (drawingBuffer*), never the attributes' idea of it
      gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
      gl.uniform2f(U.res, gl.drawingBufferWidth, gl.drawingBufferHeight); gl.uniform1f(U.t, st.still ? 0 : t); gl.uniform1f(U.flow, S.flow);
      gl.uniform1f(U.lvl, S.voice * (st.lvl || 0)); gl.uniform1f(U.think, S.think); gl.uniform1f(U.need, S.need);
      gl.uniform1f(U.gather, st.gather || 0); gl.uniform1f(U.zoom, st.zoom || 1);
      gl.uniform1f(U.done, S.done); gl.uniform1f(U.away, S.away); gl.uniform1f(U.listen, S.listen); gl.uniform1f(U.still, S.still);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      return !calm;
    }
    // fit: size the drawing buffer to the canvas without drawing (the bar keeps it right while
    // nobody sees it, so the first frame shown is never the default 300x150 buffer squeezed into
    // 64 px: amb-tools' read, m_6724)
    return { frame, fit, state: () => S };
  }

  root.AmbPearl = { create, renderer, ease, settled, start, MOODS, PINK, FS };
})(typeof window !== "undefined" ? window : globalThis);
