// Lemon Studio web build: the AudioWorklet side. GPL-3.0-or-later.
//
// "lemon-out" plays the blocks the page's pump renders (interleaved stereo, posted over the port) from a ring of a
// few blocks, and reports how many frames it played and the peak it played since the last report.
// "lemon-in" posts what the microphone records, in blocks of 512 frames (interleaved).

class LemonOut extends AudioWorkletProcessor {
  constructor() {
    super();
    this.size = 1 << 15; // frames
    this.ring = new Float32Array(this.size * 2);
    this.w = 0; this.r = 0; // frames written / read, ever
    this.played = 0; this.peak = 0; this.under = 0; this.calls = 0;
    this.port.onmessage = (e) => {
      const b = e.data;
      const n = b.length >> 1;
      for (let i = 0; i < n; i++) {
        const at = ((this.w + i) % this.size) * 2;
        this.ring[at] = b[2 * i];
        this.ring[at + 1] = b[2 * i + 1];
      }
      this.w += n;
    };
  }
  process(inputs, outputs) {
    const out = outputs[0];
    const l = out[0], r = out[1] || out[0];
    const n = l.length;
    const have = this.w - this.r;
    for (let i = 0; i < n; i++) {
      if (i < have) {
        const at = ((this.r + i) % this.size) * 2;
        const a = this.ring[at], b = this.ring[at + 1];
        l[i] = a; r[i] = b;
        const m = Math.max(Math.abs(a), Math.abs(b));
        if (m > this.peak) this.peak = m;
      } else {
        l[i] = 0; r[i] = 0;
      }
    }
    if (have < n && this.w > 0) this.under++;
    this.r += Math.min(n, have);
    this.played += n;
    if (++this.calls % 8 === 0) {
      this.port.postMessage({ read: this.r, played: this.played, peak: this.peak, under: this.under });
      this.peak = 0;
    }
    return true;
  }
}
registerProcessor('lemon-out', LemonOut);

class LemonIn extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(512);
    this.n = 0;
  }
  process(inputs) {
    const inp = inputs[0];
    if (!inp || !inp[0]) return true;
    const c0 = inp[0];
    for (let i = 0; i < c0.length; i++) {
      this.buf[this.n++] = c0[i];
      if (this.n === this.buf.length) {
        this.port.postMessage(this.buf);
        this.buf = new Float32Array(512);
        this.n = 0;
      }
    }
    return true;
  }
}
registerProcessor('lemon-in', LemonIn);
