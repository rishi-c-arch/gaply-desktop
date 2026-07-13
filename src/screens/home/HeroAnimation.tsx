// Gaply — app-home hero animation. A FAITHFUL port of the live canvas animation
// from the original design mock (Downloads/preview (2).html): drifting particles
// + rising machine-learning formula glyphs that grow, rotate, sway and fade in/
// out over the still base image, with eased pointer parallax. Ported into React
// mirroring the Globe.tsx canvas discipline (canvas ref, RAF loop, resize + DPR
// handling, cleanup on unmount — no leak). Honors prefers-reduced-motion (skips
// the loop entirely, leaving the still image visible). Frontend-only, no video.
//
// Every constant below (counts, velocities, easing, scale 0.42→1.8, sway, the
// 24-formula set) is transcribed from the original animation, not reinvented.
import React, { useEffect, useMemo, useRef } from 'react';

/** The exact machine-learning / gradient-descent formula set from the mock. */
const FORMULAS = [
  'H_out = [(H_in+2P-K)/S]+1', 'θ_j := θ_j - α ∂J(θ)/∂θ_j', '∑ ∂J(θ)/∂θ_j', 'ReLU', 'Softmax',
  '∂J(θ)=1/m Σ', 'wᵀx + b', 'σ(z)=1/(1+e⁻ᶻ)', '∂L/∂w', 'eˣ/Σeˣⁱ', 'max(0,z)', 'y = Xw', '||w||²',
  '∇J(θ)', 'J(θ)', 'h_θ(x)', 'α · ∇', 'm⁻¹ Σ L(yⁱ,ŷⁱ)', '∂/∂θ_j', 'z = wᵀx', 'a = g(z)',
  'ℒ = -Σ y log ŷ', 'softmax(z_i)', "ReLU'(z)",
];

interface Glyph {
  text: string;
  x0: number; y0: number; x1: number; y1: number;
  t: number; speed: number;
  swayAmp: number; swayFreq: number;
  rot0: number; rot1: number;
  baseSize: number; depth: number;
}
interface Particle { x: number; y: number; vx: number; vy: number; r: number; alpha: number }

const clamp = (lo: number, hi: number, x: number) => Math.max(lo, Math.min(hi, x));

/** 28 formula glyphs rising bottom-left → up-right (transcribed factory). */
function makeGlyphs(): Glyph[] {
  return Array.from({ length: 28 }).map((_, d) => {
    const K = Math.random();
    const U = (Math.random() - 0.5) * 0.18;
    const f = 0.08 + K * 0.15;          // x0 origin
    const g = 0.92 - K * 0.12 + U * 0.3; // y0 origin (near the bottom)
    const ex = f + 0.52 + Math.random() * 0.22; // x1 target (up-right)
    const vy = g - 0.48 - Math.random() * 0.22;  // y1 target (up)
    return {
      text: FORMULAS[d % FORMULAS.length],
      x0: clamp(0.02, 0.98, f + (Math.random() - 0.5) * 0.08),
      y0: clamp(0.15, 0.98, g + (Math.random() - 0.5) * 0.08),
      x1: clamp(0.02, 0.98, ex),
      y1: clamp(0.02, 0.85, vy),
      t: Math.random(),
      speed: 0.00018 + Math.random() * 0.00032,
      swayAmp: 6 + Math.random() * 18,
      swayFreq: 0.0006 + Math.random() * 0.0012,
      rot0: -8 - Math.random() * 6,
      rot1: 8 + Math.random() * 10,
      baseSize: 11 + Math.random() * 9,
      depth: 0.7 + Math.random() * 0.6,
    };
  });
}

/** 80 drifting particles (transcribed factory). */
function makeParticles(): Particle[] {
  return Array.from({ length: 80 }).map(() => ({
    x: 0.1 + Math.random() * 0.8,
    y: 0.15 + Math.random() * 0.75,
    vx: 0.00006 + Math.random() * 0.00012,
    vy: -0.00005 - Math.random() * 0.00012,
    r: 0.3 + Math.random() * 1.2,
    alpha: 0.12 + Math.random() * 0.28,
  }));
}

export interface HeroAnimationProps {
  /** dark theme → white ink + white glow (the original look); light → dark ink. */
  dark?: boolean;
}

const HeroAnimation: React.FC<HeroAnimationProps> = ({ dark = true }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const glyphs = useMemo(makeGlyphs, []);
  const particles = useMemo(makeParticles, []);
  const pointer = useRef({ x: 0, y: 0 }); // target (raw pointer)
  const smooth = useRef({ x: 0, y: 0 });  // eased parallax

  useEffect(() => {
    // Accessibility: reduced-motion → don't animate; the still image shows.
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches;
    if (reduce) return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d', { alpha: true });
    if (!ctx) return;

    // theme-aware palette (dark = the original white glow; light = readable ink)
    const ink = dark ? '#ffffff' : 'rgba(22, 30, 44, 0.9)';
    const glow = dark ? 'rgba(255,255,255,0.95)' : 'rgba(120,150,210,0.5)';
    const echo = dark ? '#cfe6ff' : '#4b6ba8';

    const onMove = (ev: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      pointer.current.x = (ev.clientX - rect.left) / rect.width - 0.5;
      pointer.current.y = (ev.clientY - rect.top) / rect.height - 0.5;
    };
    const onLeave = () => { pointer.current.x = 0; pointer.current.y = 0; };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerleave', onLeave);

    let raf = 0;
    let last = performance.now();

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const E = Math.min(33, now - last); // clamp large gaps (tab switches)
      last = now;

      // eased parallax (0.06 follow), exactly as the mock
      smooth.current.x += (pointer.current.x - smooth.current.x) * 0.06;
      smooth.current.y += (pointer.current.y - smooth.current.y) * 0.06;
      const v = smooth.current.x;
      const s = smooth.current.y;

      // DPR-correct sizing (mirrors Globe's dpr handling)
      const p = Math.max(1, window.devicePixelRatio || 1);
      const A = canvas.getBoundingClientRect();
      if (canvas.width !== Math.round(A.width * p) || canvas.height !== Math.round(A.height * p)) {
        canvas.width = Math.round(A.width * p);
        canvas.height = Math.round(A.height * p);
      }
      ctx.setTransform(p, 0, 0, p, 0, 0);
      ctx.clearRect(0, 0, A.width, A.height);

      // ── particle system: drift up-right, respawn bottom-left ──
      for (const c of particles) {
        c.x += c.vx * E;
        c.y += c.vy * E;
        if (c.x > 0.98 || c.y < 0.02) {
          c.x = 0.08 + Math.random() * 0.18;
          c.y = 0.78 + Math.random() * 0.18;
        }
      }
      ctx.save();
      for (const c of particles) {
        const px = c.x * A.width + v * 18 * c.r;
        const py = c.y * A.height + s * 18 * c.r;
        if (1 - (px / A.width) * 0.2 - (py / A.height) * 0.1 < 0) continue; // corner fade
        ctx.globalAlpha = c.alpha * 0.65;
        ctx.fillStyle = ink;
        ctx.beginPath();
        ctx.arc(px, py, c.r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();

      // ── formula glyphs: ease along path, grow, rotate, sway, fade in/out ──
      for (const c of glyphs) {
        c.t += c.speed * E;
        if (c.t > 1) {
          c.t = 0;
          const nu = (Math.random() - 0.5) * 0.06;
          c.x0 = 0.08 + Math.random() * 0.14 + nu;
          c.y0 = 0.82 + Math.random() * 0.14 + nu * 0.5;
        }
        const k = c.t;
        const ease = k < 0.5 ? 2 * k * k : -1 + (4 - 2 * k) * k; // ease-in-out quad
        const hx = c.x0 + (c.x1 - c.x0) * ease;
        const hy = c.y0 + (c.y1 - c.y0) * ease;
        const sway = Math.sin(now * c.swayFreq + c.depth * 10) * c.swayAmp * (0.4 + k);
        const gx = hx * A.width + sway + v * 12 * c.depth;
        const gy = hy * A.height + Math.cos(now * c.swayFreq * 0.8) * 4 + s * 12 * c.depth;
        const scale = 0.42 + 1.38 * k;
        const rot = (c.rot0 + (c.rot1 - c.rot0) * k) * (Math.PI / 180);
        const zS = Math.sin(k * Math.PI);
        const alpha = Math.pow(zS, 0.85) * (0.72 + c.depth * 0.18);
        if (alpha < 0.01) continue;

        ctx.save();
        ctx.translate(gx, gy);
        ctx.rotate(rot);
        ctx.scale(scale, scale);
        const short = c.text.length <= 6;
        ctx.font = `${short ? '700' : '500'} ${c.baseSize}px "JetBrains Mono", "SF Mono", Menlo, monospace`;
        ctx.shadowColor = glow;
        ctx.shadowBlur = 14 * scale;
        ctx.globalAlpha = alpha;
        ctx.fillStyle = ink;
        ctx.fillText(c.text, 0, 0);
        // second pass without the blur for a crisp core
        ctx.shadowBlur = 0;
        ctx.globalAlpha = alpha * 0.92;
        ctx.fillText(c.text, 0, 0);
        // faint chromatic echo on the larger long glyphs
        if (scale > 1.1 && !short) {
          ctx.globalAlpha = alpha * 0.18;
          ctx.fillStyle = echo;
          ctx.fillText(c.text, 0.6, 1.2);
        }
        ctx.restore();
      }
    };

    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerleave', onLeave);
    };
  }, [dark, glyphs, particles]);

  return <canvas ref={canvasRef} className="home-hero-clip__canvas" aria-hidden="true" data-testid="home-hero-animation" />;
};

export default HeroAnimation;
