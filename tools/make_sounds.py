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


def coin_button():
    """A small brass button bounces twice on the wooden counter."""
    out = np.zeros(int(0.15 * SR))
    partials = [(1.0, 1.0, 0.012), (2.6, 0.44, 0.009), (4.1, 0.22, 0.005)]
    place(out, tone(1040, 0.09, partials, attack=0.0008), 0)
    place(out, 0.45 * tone(1230, 0.08, partials, attack=0.001), 0.052)
    return normalise(out, 0.45)


def coin_candle():
    """A wax stub lands with a soft thud and slides a little."""
    t = t_axis(0.19)
    out = tone(190, 0.19, [(1.0, 1.0, 0.028), (2.4, 0.32, 0.012)], attack=0.003)
    grain = np.random.default_rng(43).standard_normal(len(t))
    slide = normalise(bandpass_fft(grain, 280, 1150), 1.0)
    out += 0.27 * slide * np.sin(np.pi * t / 0.19) ** 2
    return normalise(out, 0.44)


def coin_spoon():
    """Silver rings after its bowl strikes the edge of the tray."""
    partials = [(1.0, 1.0, 0.13), (2.37, 0.40, 0.067), (3.84, 0.19, 0.031)]
    return normalise(tone(1810, 0.55, partials, attack=0.001), 0.33)


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


def intro_music():
    """Four slow bars for the punt, with the shop's music box answering the bowed bass."""
    out = np.zeros(14 * SR)
    bars = [(45, 64, 69), (41, 60, 64), (50, 62, 65), (40, 59, 68)]
    for bar, (bass, fifth, top) in enumerate(bars):
        at = 0.12 + 3 * bar
        t = t_axis(3.38)
        freq = midi_hz(bass) * (1 + 0.002 * np.sin(2 * np.pi * 4.3 * t))
        phase = 2 * np.pi * np.cumsum(freq) / SR
        bow = np.sin(phase) + 0.33 * np.sin(2 * phase) + 0.14 * np.sin(3 * phase)
        envelope = np.interp(t, [0, 0.34, 1.1, 2.64, 3.38], [0, 0.8, 0.68, 0.57, 0])
        place(out, 0.24 * bow * envelope, at)
        for delay, note in [(0.08, fifth), (1.02, top), (2.04, fifth)]:
            place(out, music_box_note(note, 0.22), at + delay)

    # Leave space between the upper notes for the three spoken lines.
    for at, note in [(0.36, 76), (4.34, 72), (7.94, 74), (11.85, 71), (12.49, 69)]:
        place(out, music_box_note(note, 0.31), at)
    place(out, music_box_note(45, 0.54), 12.28)
    score = reverb(out, 0.24)
    fade = int(0.95 * SR)
    score[-fade:] *= np.linspace(1, 0, fade) ** 2
    return normalise(score, 0.40)


def music_box_loop(melody, harmony, bpm, shift=12, bass_shift=12, wet=0.5, extra=None):
    """A looping 3/4 music box tune: melody notes on the beat, bass on one,
    chord on two and three. `extra(out, bar, start, beat)` adds more per bar."""
    beat = 60 / bpm
    loop = int(len(melody) * 3 * beat * SR)
    out = np.zeros(loop + 4 * SR)
    for bar, notes in enumerate(melody):
        start = bar * 3 * beat
        at = start
        for note, beats in notes:
            place(out, music_box_note(note + shift, 0.9), at)
            at += beats * beat
        bass, chord = harmony[bar]
        place(out, music_box_note(bass + bass_shift, 0.5), start)
        for b in (1, 2):
            for c in chord:
                place(out, music_box_note(c + shift, 0.22), start + b * beat)
        if extra:
            extra(out, bar, start, beat)
    out = reverb(out, wet)
    # Wrap the tail onto the start so the loop is seamless.
    looped = out[:loop].copy()
    tail = out[loop:]
    looped[: len(tail)] += tail[: len(looped)]
    return normalise(looped, 0.35)


def evening_music():
    return music_box_loop(MELODY, HARMONY, 76)


# Season 2, the long winter: a slower waltz in D minor with sleigh bells.
WINTER_MELODY = [
    [(69, 1.5), (67, 0.5), (65, 1)], [(62, 2), (65, 1)], [(64, 1), (65, 1), (67, 1)], [(69, 3)],
    [(70, 1.5), (69, 0.5), (67, 1)], [(65, 2), (62, 1)], [(64, 1), (61, 1), (64, 1)], [(62, 3)],
    [(65, 1.5), (67, 0.5), (69, 1)], [(74, 2), (72, 1)], [(70, 1), (69, 1), (67, 1)], [(69, 3)],
    [(70, 1.5), (69, 0.5), (67, 1)], [(65, 2), (64, 1)], [(62, 1), (61, 1), (64, 1)], [(62, 3)],
]
WINTER_HARMONY = [
    (50, (62, 65)), (46, (62, 65)), (48, (64, 67)), (45, (61, 64)),
    (43, (62, 67)), (50, (62, 65)), (45, (61, 64)), (50, (62, 65)),
    (53, (65, 69)), (46, (65, 70)), (43, (62, 67)), (45, (61, 64)),
    (43, (62, 67)), (50, (62, 65)), (45, (61, 64)), (50, (62, 65)),
]

# Season 3, the thaw: a brighter, quicker tune in G major with birdsong.
SPRING_MELODY = [
    [(71, 1), (74, 1), (79, 1)], [(78, 2), (76, 1)], [(74, 1), (72, 1), (71, 1)], [(69, 3)],
    [(72, 1), (76, 1), (79, 1)], [(81, 2), (79, 1)], [(78, 1), (76, 1), (74, 1)], [(74, 3)],
    [(71, 1), (74, 1), (79, 1)], [(83, 2), (81, 1)], [(79, 1), (76, 1), (72, 1)], [(74, 3)],
    [(76, 1), (78, 1), (79, 1)], [(81, 1.5), (79, 0.5), (76, 1)], [(74, 1), (72, 1), (69, 1)], [(67, 3)],
]
SPRING_HARMONY = [
    (43, (59, 62)), (50, (62, 66)), (43, (59, 62)), (50, (62, 66)),
    (48, (60, 64)), (50, (62, 66)), (50, (62, 66)), (43, (59, 62)),
    (43, (59, 62)), (52, (59, 64)), (48, (60, 64)), (50, (62, 66)),
    (48, (60, 64)), (50, (62, 66)), (50, (60, 66)), (43, (59, 62)),
]


def sleigh_bells(seconds, seed):
    """A soft shake of tiny bells: bright, quickly fading noise."""
    local = np.random.default_rng(seed)
    t = t_axis(seconds)
    noise = bandpass_fft(local.standard_normal(len(t)), 5000, 9000)
    jingle = sum(np.sin(2 * np.pi * f * t) for f in (4186, 5274, 6272)) * 0.15
    return (normalise(noise, 1.0) + jingle) * np.exp(-t / 0.09)


def bird_chirp(start_hz, end_hz, seconds=0.11):
    t = t_axis(seconds)
    freq = np.linspace(start_hz, end_hz, len(t))
    phase = 2 * np.pi * np.cumsum(freq) / SR
    return np.sin(phase) * np.sin(np.pi * t / seconds) ** 2


def evening_music_winter():
    def bells(out, bar, start, beat):
        for b in (1, 2):
            place(out, 0.05 * sleigh_bells(0.35, bar * 3 + b), start + b * beat)

    return music_box_loop(WINTER_MELODY, WINTER_HARMONY, 66, wet=0.6, extra=bells)


def evening_music_spring():
    def birds(out, bar, start, beat):
        # Now and then a little bird answers the tune.
        if bar % 4 == 3:
            for i, (a, b) in enumerate([(3200, 4200), (3400, 4600), (4400, 3000)]):
                place(out, 0.06 * bird_chirp(a, b), start + 1.2 * beat + i * 0.16)

    return music_box_loop(SPRING_MELODY, SPRING_HARMONY, 88, shift=0, wet=0.4, extra=birds)


def wind(seconds, cutoff, swells):
    """Low-passed noise with slow swells (whole cycles per loop: seamless)."""
    n = seconds * SR
    t = np.arange(n) / SR
    air = normalise(lowpass_fft(rng.standard_normal(n), cutoff, order=2), 1.0)
    swell = 0.55 + sum(a * np.sin(2 * np.pi * k / seconds * t + p) for k, a, p in swells)
    return air * swell, t


def night_ambience():
    seconds = 40
    air, t = wind(seconds, 350, [(2, 0.3, 0.0), (5, 0.15, 1.3)])
    # A faint, slowly beating drone.
    drone = 0.12 * np.sin(2 * np.pi * (2200 / seconds) * t) + 0.08 * np.sin(2 * np.pi * (3302 / seconds) * t)
    drone *= 0.6 + 0.4 * np.sin(2 * np.pi * 3 / seconds * t)
    return normalise(air * 0.8 + drone, 0.3)


def night_ambience_winter():
    """Colder, gustier wind that whistles round the corners."""
    seconds = 40
    air, t = wind(seconds, 650, [(3, 0.35, 0.4), (7, 0.2, 2.1)])
    whistle_noise = bandpass_fft(rng.standard_normal(len(t)), 700, 1100)
    gusts = np.clip(np.sin(2 * np.pi * 4 / seconds * t + 0.7), 0, 1) ** 3
    whistle = normalise(whistle_noise, 1.0) * gusts * 0.25
    return normalise(air * 0.8 + whistle, 0.3)


def night_ambience_spring():
    """A mild breeze, crickets in the grass and a frog now and then."""
    seconds = 40
    air, t = wind(seconds, 260, [(2, 0.2, 0.0), (3, 0.1, 0.9)])
    crickets = np.zeros(len(t))
    chirp = bird_chirp(4600, 4700, 0.035)
    for i in range(seconds * 2):
        at = i * 0.5
        loud = 0.6 + 0.4 * np.sin(2 * np.pi * 3 * at / seconds)
        for k in range(3):
            place(crickets, loud * chirp, at + k * 0.06)
    frogs = np.zeros(len(t))
    for at in (6.0, 6.45, 23.5, 31.0, 31.4):
        ft = t_axis(0.22)
        croak = np.sin(2 * np.pi * 180 * ft) * (0.5 + 0.5 * np.sign(np.sin(2 * np.pi * 30 * ft)))
        place(frogs, croak * np.sin(np.pi * ft / 0.22), at)
    return normalise(air * 0.6 + 0.08 * crickets + 0.12 * lowpass_fft(frogs, 900), 0.3)


def clock_click(freq, seed, length=0.08):
    """One tick of a tower clock: a short knock with a ringing body."""
    local = np.random.default_rng(seed)
    t = t_axis(length)
    knock = bandpass_fft(local.standard_normal(len(t)), freq * 0.6, freq * 1.6)
    body = np.sin(2 * np.pi * freq * t) * 0.6
    return (normalise(knock, 1.0) + body) * np.exp(-t / 0.018)


def clock_loop(times, seconds):
    """A seamless loop with a tick (high) or tock (low) at each time."""
    out = np.zeros(int(seconds * SR))
    for i, at in enumerate(times):
        freq = 2100 if i % 2 == 0 else 1500
        place(out, clock_click(freq, i), at)
    return normalise(reverb(out, 0.35), 0.28)


def clock_tick_slow():
    """Just wound: the works catch and stumble, slow and uneven."""
    return clock_loop([0.0, 1.55, 2.25, 4.1], 5.0)


def clock_tick():
    """Set right: a steady tick-tock, once a second."""
    return clock_loop([0.0, 1.0], 2.0)


def tower_bell(freq, seconds=4.5):
    partials = [(1.0, 1.0, 2.6), (2.0, 0.55, 1.6), (2.76, 0.35, 1.1), (5.4, 0.18, 0.5), (0.5, 0.4, 3.2)]
    return tone(freq, seconds, partials, attack=0.004)


def clock_strike():
    """Twelve strokes of the tower bell, the first time it strikes again."""
    stroke = 1.25
    out = np.zeros(int((12 * stroke + 4.5) * SR))
    for i in range(12):
        place(out, 0.8 * tower_bell(midi_hz(50)), i * stroke)
    return normalise(reverb(out, 0.45), 0.45)


SOUNDS = {
    "bell": bell,
    "coins": coins,
    "coin_button": coin_button,
    "coin_candle": coin_candle,
    "coin_spoon": coin_spoon,
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
    "intro_music": intro_music,
    "evening_music": evening_music,
    "night_ambience": night_ambience,
    "evening_music_winter": evening_music_winter,
    "night_ambience_winter": night_ambience_winter,
    "evening_music_spring": evening_music_spring,
    "night_ambience_spring": night_ambience_spring,
    "clock_tick_slow": clock_tick_slow,
    "clock_tick": clock_tick,
    "clock_strike": clock_strike,
}


LOOPS = {
    "evening_music",
    "night_ambience",
    "evening_music_winter",
    "night_ambience_winter",
    "evening_music_spring",
    "night_ambience_spring",
    "clock_tick_slow",
    "clock_tick",
}


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
