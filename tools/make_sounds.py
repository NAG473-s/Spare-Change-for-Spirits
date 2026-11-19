#!/usr/bin/env python3
"""Synthesises the game's sound effects and music into assets/sounds/*.ogg.

Everything is generated from scratch (no samples), so there are no licence
questions. Re-run after changing anything here; replace any file with your
own recording of the same name to override it.

    python3 tools/make_sounds.py

Needs numpy and ffmpeg (with libvorbis).
"""

import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "sounds"
SR = 44100
rng = np.random.default_rng(7)


def t_axis(seconds: float) -> np.ndarray:
    return np.arange(int(seconds * SR)) / SR


def midi_hz(note: float) -> float:
    return 440.0 * 2 ** ((note - 69) / 12)


def tone(freq, seconds, partials, attack=0.002):
    """Sum of decaying sine partials: [(ratio, amplitude, decay seconds)]."""
    t = t_axis(seconds)
    out = np.zeros_like(t)
    for ratio, amp, decay in partials:
        out += amp * np.sin(2 * np.pi * freq * ratio * t) * np.exp(-t / decay)
    out *= np.minimum(1.0, t / attack)
    return out


def place(buf, sound, at):
    i = int(at * SR)
    end = min(len(buf), i + len(sound))
    buf[i:end] += sound[: end - i]


def reverb(x, wet=0.3, taps=((0.037, 0.5), (0.053, 0.42), (0.071, 0.35), (0.113, 0.25))):
    """Cheap feedback-comb reverb, good enough for a soft room tail."""
    out = x.copy()
    for delay, gain in taps:
        d = int(delay * SR)
        y = x.copy()
        for k in range(1, 12):
            shift = d * k
            if shift >= len(x):
                break
            y[shift:] += x[:-shift] * gain**k
        out += wet * (y - x)
    return out


def lowpass_fft(x, cutoff, order=2):
    """Zero-phase low-pass in the frequency domain (periodic, so loops stay seamless)."""
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    spec /= np.sqrt(1 + (f / cutoff) ** (2 * order))
    return np.fft.irfft(spec, n=len(x))


def normalise(x, peak):
    return x / (np.max(np.abs(x)) + 1e-9) * peak


def bell():
    out = np.zeros(int(2.4 * SR))
    f = midi_hz(91)  # G6
    partials = [(1.0, 1.0, 1.4), (2.0, 0.35, 0.9), (2.76, 0.5, 0.7), (5.4, 0.25, 0.35), (8.93, 0.12, 0.2)]
    place(out, tone(f, 2.2, partials), 0.0)
    place(out, 0.7 * tone(f * 1.003, 2.0, partials), 0.16)
    return normalise(reverb(out, 0.25), 0.55)


def midnight():
    """The shop clock strikes midnight: three deep, slow strikes."""
    out = np.zeros(int(4.6 * SR))
    f = midi_hz(48)  # C3
    partials = [(1.0, 1.0, 2.2), (2.0, 0.5, 1.5), (2.4, 0.35, 1.1), (3.0, 0.3, 0.9), (4.2, 0.15, 0.5), (5.4, 0.1, 0.3)]
    for i in range(3):
        place(out, tone(f, 2.6, partials, attack=0.003), i * 0.95)
    return normalise(reverb(out, 0.35), 0.6)


def coins():
    out = np.zeros(int(0.7 * SR))
    for at in (0.0, 0.06, 0.13, 0.19):
        base = rng.uniform(3000, 3600)
        partials = [(r * rng.uniform(0.97, 1.03), a, d) for r, a, d in
                    ((1.0, 1.0, 0.12), (1.47, 0.6, 0.09), (2.09, 0.45, 0.06), (2.71, 0.3, 0.04))]
        place(out, rng.uniform(0.6, 1.0) * tone(base, 0.3, partials, attack=0.0005), at)
    return normalise(reverb(out, 0.15), 0.45)


def chime():
    out = np.zeros(int(3.0 * SR))
    for i, note in enumerate((76, 80, 83, 88)):  # E major arpeggio
        partials = [(1.0, 1.0, 1.2), (2.0, 0.25, 0.6), (3.0, 0.08, 0.3)]
        place(out, tone(midi_hz(note), 2.2, partials, attack=0.01), i * 0.13)
    return normalise(reverb(out, 0.45), 0.5)


def crash():
    out = np.zeros(int(0.9 * SR))
    t = t_axis(0.35)
    noise = rng.standard_normal(len(t)) * np.exp(-t / 0.06)
    noise = np.diff(noise, prepend=0.0)  # brighter
    place(out, noise * 0.5, 0.0)
    for _ in range(5):
        f = rng.uniform(2500, 6000)
        partials = [(1.0, 1.0, 0.15), (1.6, 0.5, 0.1), (2.3, 0.3, 0.06)]
        place(out, rng.uniform(0.3, 0.7) * tone(f, 0.4, partials, attack=0.0005), rng.uniform(0.02, 0.3))
    return normalise(reverb(out, 0.2), 0.5)


def click():
    return normalise(tone(1800, 0.05, [(1.0, 1.0, 0.012), (2.5, 0.3, 0.006)], attack=0.0005), 0.18)


def sigh():
    """Disappointed ghost: two soft falling notes with vibrato."""
    out = np.zeros(int(1.0 * SR))
    for i, note in enumerate((69, 65)):
        t = t_axis(0.5)
        f = midi_hz(note) * (1 + 0.012 * np.sin(2 * np.pi * 5.5 * t))
        phase = 2 * np.pi * np.cumsum(f) / SR
        env = np.minimum(1, t / 0.08) * np.exp(-t / 0.35)
        place(out, (np.sin(phase) + 0.2 * np.sin(2 * phase)) * env, i * 0.28)
    return normalise(reverb(out, 0.3), 0.3)


def bandpass_fft(x, low, high):
    """Soft band-pass in the frequency domain."""
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    spec /= np.sqrt(1 + (f / high) ** 4) * np.sqrt(1 + (low / np.maximum(f, 1)) ** 4)
    return np.fft.irfft(spec, n=len(x))


def ghost_voice(contour, seconds, base=midi_hz(62), breath=0.25, seed=1):
    """A soft, hollow "oooo": a few low partials with slow vibrato that
    follow a pitch contour (semitones over time), wrapped in breath."""
    t = t_axis(seconds)
    semis = np.interp(t / seconds, np.linspace(0, 1, len(contour)), contour)
    vibrato = 0.25 * np.sin(2 * np.pi * 4.5 * t) * np.minimum(1, t / 0.4)
    f = base * 2 ** ((semis + vibrato) / 12)
    phase = 2 * np.pi * np.cumsum(f) / SR
    # Hollow "oo" vowel: strong fundamental, weak odd partials.
    voice = np.sin(phase) + 0.18 * np.sin(3 * phase) + 0.05 * np.sin(5 * phase)
    # Own random generator, so these sounds never shift the others.
    air = bandpass_fft(np.random.default_rng(seed).standard_normal(len(t)), 300, 1600)
    air = normalise(air, 1.0) * breath
    env = np.minimum(1, t / (0.35 * seconds)) * np.minimum(1, (seconds - t) / (0.45 * seconds))
    env = np.clip(env, 0, 1) ** 1.5
    return (voice + air) * env


def ghost_whoo_1():
    """Rising, then falling "wooooo"."""
    return normalise(reverb(ghost_voice([0, 4, 7, 5, 0, -3], 1.6), 0.45), 0.32)


def ghost_whoo_2():
    """A long, sinking "ooooh"."""
    return normalise(reverb(ghost_voice([5, 4, 2, 0, -4], 1.4, seed=2), 0.45), 0.3)


def ghost_whoo_3():
    """Two short hoots, like a question: "oo-ooo?"."""
    out = np.zeros(int(1.7 * SR))
    place(out, ghost_voice([0, 1], 0.55, breath=0.3, seed=3), 0.0)
    place(out, ghost_voice([-2, 0, 5], 0.8, breath=0.3, seed=4), 0.6)
    return normalise(reverb(out, 0.45), 0.3)


def ghost_whisper():
    """Breathy, unvoiced syllables – a wish whispered across the counter."""
    seconds = 1.3
    t = t_axis(seconds)
    g = np.random.default_rng(5)
    hiss = bandpass_fft(g.standard_normal(len(t)), 1200, 5200)
    hiss = normalise(hiss, 1.0)
    env = np.zeros_like(t)
    at = 0.05
    while at < seconds - 0.15:
        length = g.uniform(0.09, 0.2)
        level = g.uniform(0.5, 1.0)
        k = (t - at) / length
        env += level * np.where((k >= 0) & (k <= 1), np.sin(np.pi * np.clip(k, 0, 1)) ** 2, 0)
        at += length + g.uniform(0.02, 0.08)
    # A faint, low "voice" under the hiss keeps it from sounding like steam.
    murmur = 0.12 * np.sin(2 * np.pi * midi_hz(55) * t) * env
    return normalise(reverb(hiss * env + murmur, 0.35), 0.34)


def ghost_hum():
    """A very soft, slowly beating hum from somewhere between the shelves."""
    seconds = 2.2
    t = t_axis(seconds)
    f = midi_hz(57)
    hum = np.sin(2 * np.pi * f * t) + 0.8 * np.sin(2 * np.pi * (f * 1.006) * t)
    hum += 0.15 * np.sin(2 * np.pi * 2 * f * t)
    env = np.sin(np.pi * t / seconds) ** 2
    return normalise(reverb(hum * env, 0.4), 0.18)


def ghost_flee():
    """A startled "ooh?" that glides down and fades away."""
    return normalise(reverb(ghost_voice([7, 8, 3, -5, -12], 1.2, breath=0.5, seed=6), 0.5), 0.3)


# A small waltz in A minor for the music box: (midi, beats) per bar.
MELODY = [
    [(76, 1.5), (74, 0.5), (72, 1)], [(72, 2), (69, 1)], [(67, 1), (72, 1), (76, 1)], [(74, 3)],
    [(76, 1.5), (77, 0.5), (76, 1)], [(74, 2), (77, 1)], [(71, 1), (68, 1), (71, 1)], [(69, 3)],
    [(72, 1.5), (74, 0.5), (76, 1)], [(79, 2), (77, 1)], [(76, 1), (72, 1), (67, 1)], [(69, 3)],
    [(74, 1.5), (76, 0.5), (77, 1)], [(76, 2), (71, 1)], [(72, 1), (71, 1), (68, 1)], [(69, 3)],
]
# (bass, chord tones) per bar
HARMONY = [
    (57, (64, 69)), (53, (65, 69)), (60, (64, 67)), (52, (64, 68)),
    (57, (64, 69)), (50, (65, 69)), (52, (64, 68)), (57, (64, 69)),
    (53, (65, 69)), (55, (62, 67)), (60, (64, 67)), (57, (64, 69)),
    (50, (65, 69)), (52, (64, 68)), (57, (64, 69)), (57, (64, 69)),
]


def music_box_note(note, amp):
    partials = [(1.0, 1.0, 1.6), (2.0, 0.35, 0.5), (3.0, 0.12, 0.25), (4.17, 0.05, 0.08)]
    return amp * tone(midi_hz(note), 2.5, partials, attack=0.003)


def evening_music():
    beat = 60 / 76
    loop = int(len(MELODY) * 3 * beat * SR)
    out = np.zeros(loop + 4 * SR)
    for bar, notes in enumerate(MELODY):
        start = bar * 3 * beat
        at = start
        for note, beats in notes:
            place(out, music_box_note(note + 12, 0.9), at)
            at += beats * beat
        bass, chord = HARMONY[bar]
        place(out, music_box_note(bass + 12, 0.5), start)
        for b in (1, 2):
            for c in chord:
                place(out, music_box_note(c + 12, 0.22), start + b * beat)
    out = reverb(out, 0.5)
    # Wrap the tail onto the start so the loop is seamless.
    looped = out[:loop].copy()
    tail = out[loop:]
    looped[: len(tail)] += tail[: len(looped)]
    return normalise(looped, 0.35)


def night_ambience():
    seconds = 40
    n = seconds * SR
    t = np.arange(n) / SR
    # Wind: low-passed noise with slow swells. All modulation frequencies are
    # whole cycles per loop, so the loop is seamless.
    wind = lowpass_fft(rng.standard_normal(n), 350, order=2)
    swell = 0.55 + 0.3 * np.sin(2 * np.pi * 2 / seconds * t) + 0.15 * np.sin(2 * np.pi * 5 / seconds * t + 1.3)
    wind = normalise(wind, 1.0) * swell
    # A faint, slowly beating drone.
    drone = 0.12 * np.sin(2 * np.pi * (2200 / seconds) * t) + 0.08 * np.sin(2 * np.pi * (3302 / seconds) * t)
    drone *= 0.6 + 0.4 * np.sin(2 * np.pi * 3 / seconds * t)
    return normalise(wind * 0.8 + drone, 0.3)


SOUNDS = {
    "bell": bell,
    "coins": coins,
    "chime": chime,
    "midnight": midnight,
    "crash": crash,
    "click": click,
    "sigh": sigh,
    "ghost_whoo_1": ghost_whoo_1,
    "ghost_whoo_2": ghost_whoo_2,
    "ghost_whoo_3": ghost_whoo_3,
    "ghost_whisper": ghost_whisper,
    "ghost_hum": ghost_hum,
    "ghost_flee": ghost_flee,
    "evening_music": evening_music,
    "night_ambience": night_ambience,
}


LOOPS = {"evening_music", "night_ambience"}


def write_ogg(name, samples):
    OUT.mkdir(parents=True, exist_ok=True)
    if name not in LOOPS:
        # Short fade-out so one-shots never end with a click.
        n = int(0.03 * SR)
        samples = samples.copy()
        samples[-n:] *= np.linspace(1, 0, n)
    pcm = (np.clip(samples, -1, 1) * 32767).astype(np.int16)
    with tempfile.TemporaryDirectory() as tmp:
        wav = Path(tmp) / f"{name}.wav"
        with wave.open(str(wav), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes(pcm.tobytes())
        out = OUT / f"{name}.ogg"
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), "-c:a", "libvorbis", "-q:a", "4", str(out)],
            check=True,
        )
    print(f"wrote {out.relative_to(ROOT)} ({len(samples) / SR:.1f} s)")


def main() -> int:
    names = sys.argv[1:] or list(SOUNDS)
    for name in names:
        write_ogg(name, SOUNDS[name]())
    return 0


if __name__ == "__main__":
    sys.exit(main())
