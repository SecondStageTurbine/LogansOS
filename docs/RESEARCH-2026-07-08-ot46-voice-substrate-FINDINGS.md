<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-46(B) Arthur's voice substrate — model & integration findings

**Date:** 2026-07-08
**Author:** Prometheus (research lane, host-only)
**Branch:** `prometheus/ot46-voice-substrate-research`
**Dispatch:** `Plans/DISPATCH-2026-07-08-ot46-voice-substrate-research-prometheus.md`
**Scope:** Research + FINDINGS doc. No code, no QEMU. Voice stays Phase-N, gated on bare-metal bringup + PL ratification.

---

## Executive summary

On-device bidirectional voice **is feasible** on our substrate. STT-in is the load-bearing half (Article 1.5: a five-year-old speaks before typing). **Moonshine Small** is the recommended STT — purpose-built for live, streaming voice commands, 8.1x faster than Whisper Tiny on identical hardware, with built-in intent recognition. For TTS-out, **Kokoro** (StyleTTS2, 82M params, Apache 2.0) with British male voices (`bm_george`/`bm_fable`) is the recommended neural option for the butler persona; **Piper** (VITS/ONNX, GPL-3.0) is the smallest-footprint alternative if copyleft is acceptable. The substrate-integration verdict: **userland inference process** — voice models are architecturally distinct from our decoder-only Qwen forward and should NOT ride OT-16's data-driven forward graph. Privacy architecture: push-to-talk via explicit key/gesture, never wake-word; the whole path is on-device by construction.

---

## Q1 — STT (speech→text): the load-bearing half

### Candidate survey

| Model | Architecture | Smallest viable size | CPU latency (Linux x86) | WER (short-form/OOD) | License | Reference impl |
|-------|-------------|---------------------|------------------------|---------------------|---------|---------------|
| **Moonshine Tiny** | Custom streaming transformer, from scratch | 34M params (~136 MB FP32) | **69ms** latency | 12.00% | [Apache 2.0](https://github.com/moonshine-ai/moonshine/blob/main/LICENSE) | Yes — portable C++ core, OnnxRuntime |
| **Moonshine Small** | Same | 123M params (~492 MB FP32) | **165ms** latency | **7.84%** | Apache 2.0 | Yes |
| **Moonshine Medium** | Same | 245M params (~980 MB FP32) | 269ms latency | 6.65% | Apache 2.0 | Yes |
| **Whisper.cpp Tiny** | Encoder-decoder transformer (OpenAI Whisper) | 75 MB (GGML quantized) | Encoder: 422ms (Ryzen 9/8t) | Not published for commands | MIT | Yes — whisper.cpp C++ repo |
| **Whisper.cpp Base** | Same | 142 MB | Encoder: 880ms (Ryzen 9/8t) | Not published | MIT | Yes |
| **Distil-Whisper Small.en** | Distilled encoder-decoder (fixed encoder, reduced decoder) | 166M params (~330 MB FP16) | No CPU benchmarks | 12.1% (short-form OOD) | MIT | Yes — but PyTorch-only, no native CPU opt |
| **Vosk Small (en-us)** | TDNN nnet3 + i-vectors (Kaldi DNN, not transformer) | **40 MB** | "Zero-latency streaming" (no ms data) | 9.85% (LibriSpeech test-clean) | Apache 2.0 | Yes — vosk-api C++ |
| **Faster-Whisper** | Whisper via CTranslate2 | Same as Whisper models | 1m42s for 13 min audio (int8, i7-12700K) | Same as Whisper | MIT | Yes — but GPU-focused |

### Sources

- **Moonshine:** [README benchmark table](https://github.com/moonshine-ai/moonshine) — latency numbers from the "Benchmark" section (MacBook Pro, Linux x86, Raspberry Pi 5). WER from the same table. Architecture: "All speech to text models are based on our cutting edge research and trained from scratch." Intent recognition: [IntentRecognizer section](https://github.com/moonshine-ai/moonshine) — built-in semantic intent matching for commands.
- **Whisper.cpp:** [README Memory usage table](https://github.com/ggml-org/whisper.cpp) — model sizes. [Issue #89 benchmark](https://github.com/ggml-org/whisper.cpp/issues/89) — encoder latency on M1 Pro, Ryzen 9 3900X, Raspberry Pi 4. Command example: [examples/command](https://github.com/ggml-org/whisper.cpp/blob/master/examples/command).
- **Distil-Whisper:** [Model card](https://huggingface.co/distil-whisper/distil-small.en) — 166M params, 5.6x speedup vs Whisper small, WER 12.1% short-form OOD, 3.43% LibriSpeech test-clean. No CPU latency benchmarks (all GPU).
- **Vosk:** [README](https://github.com/alphacep/vosk-api) — "zero-latency response with streaming API." [Models page](https://alphacephei.com/vosk/models) — small-en-us-0.15 at 40 MB, WER 9.85% LibriSpeech test-clean, 10.38% TEDLIUM.
- **Faster-Whisper:** [README benchmarks](https://github.com/SYSTRAN/faster-whisper) — CPU/GPU tables.

### Recommendation: Moonshine Small

**Rationale:** Voice commands are short utterances (1-5 seconds), not dictation. Moonshine is purpose-built for this regime:

1. **Latency is decisive.** 165ms on Linux x86 vs 880ms+ for Whisper.cpp Base. For a five-year-old talking to Arthur, sub-200ms response is the difference between "it's listening" and "is it broken." Moonshine Tiny at 69ms is even faster but 12% WER is borderline for command accuracy. Small at 7.84% WER is the sweet spot.
2. **Built-in intent recognition.** Moonshine ships with an `IntentRecognizer` — semantic matching for voice commands, not just raw transcription. This is directly useful for Arthur's command-dispatch layer.
3. **Streaming-native.** Caching for streaming, flexible input windows. Whisper.cpp has streaming examples but they're bolted on; Moonshine was designed for it.
4. **CPU-only, no GPU dependency.** OnnxRuntime backend, runs on Raspberry Pi 5 (527ms for Small). No CUDA, no PyTorch.
5. **License.** Apache 2.0 — permissive, no copyleft concerns.
6. **Reference implementation.** Portable C++ core library — a correct reference exists for the parity oracle (OT-30 discipline).

**Why not Whisper.cpp:** It's proven and widely used, but architecturally heavier (encoder-decoder transformer) and slower for short utterances. It's the right choice for long-form transcription; for voice commands, Moonshine is 8.1x faster at comparable accuracy.

**Why not Vosk:** Smallest footprint (40 MB) but uses the older Kaldi TDNN architecture, not a transformer. WER is acceptable (9.85%) but no modern streaming latency benchmarks. It's a fallback if model size is the hard constraint (<100 MB total).

**Why not Distil-Whisper:** No CPU benchmarks, requires PyTorch (heavy runtime), and the smallest variant (distil-small.en) has 12.1% WER — worse than Moonshine Small at 7.84%. The 5.6x speedup is relative to Whisper small (already slow), not an absolute advantage.

---

## Q2 — TTS (text→speech): the output half + the real fork

### Candidate survey

| Model | Architecture | Model size | CPU viability | British EN voices | Butler fit | License | Reference impl |
|-------|-------------|-----------|--------------|-------------------|------------|---------|---------------|
| **Kokoro** | StyleTTS2 + ISTFTNet vocoder, decoder-only | 82M params (~328 MB FP32) + tiny voice tensors (~10-50 KB each) | Yes — PyTorch, no GPU required | 4F + 4M (`bf_`/`bm_` voices) | **Good** — `bm_george`, `bm_fable` are British male | Apache 2.0 | Yes — hexgrad/kokoro |
| **Piper (en_GB alan, medium)** | VITS, ONNX export | ~50 MB/voice | **Excellent** — runs on Pi 4 | 9 voices including `alan`, `northern_english_male` | Medium — depends on training data | GPL-3.0 | Yes — OHF-Voice/piper1-gpl |
| **Piper (en_GB alan, low)** | Same | ~20 MB/voice | Excellent | Same voices | Lower quality | GPL-3.0 | Yes |
| **eSpeak-NG** | Formant synthesis (not neural) | ~5-10 MB total binary+data | Instant | `en-gb` voice | **Poor** — robotic | GPL-3.0+ | Yes |

### Sources

- **Kokoro:** [README](https://github.com/hexgrad/kokoro) — 82M params, StyleTTS2 + ISTFTNet, "significantly faster and more cost-efficient." [VOICES.md](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md) — British voices: `bf_alice`, `bf_emma`, `bf_isabella`, `bf_lily`, `bm_daniel`, `bm_fable`, `bm_george`, `bm_lewis`. British male grades: C-D (MM minutes training).
- **Piper:** [README](https://github.com/OHF-Voice/piper1-gpl) — VITS/ONNX, "fast and local," runs on Pi 3/4. [Voice list](https://huggingface.co/rhasspy/piper-voices/tree/main/en/en_GB) — 9 en_GB voices. Model sizes from voice card metadata (low ~10-20 MB, medium ~20-50 MB).
- **eSpeak-NG:** [README](https://github.com/espeak-ng/espeak-ng) — "The speech is clear, and can be used at high speeds, but is not as natural or smooth as larger synthesizers which are based on human speech recordings."

### The real fork: Neural vs Classical

This is a **genuine architectural fork**, not a preference:

- **Neural (Kokoro/Piper):** Natural prosody, warmth, butler-appropriate tone. Requires a neural inference runtime (PyTorch or ONNX). Model sizes: 20 MB (Piper low) to ~328 MB (Kokoro). Introduces a new forward path if run in-kernel; clean as a userland process.
- **Classical (eSpeak-NG):** Deterministic, tiny (~5-10 MB), no neural forward at all. Robotic voice — fails the butler persona test. The README is explicit: "not as natural or smooth."

### Recommendation: Kokoro for the butler voice

**Rationale:**

1. **The butler persona needs warmth.** Rule 8 of `VISION-arthur-persona.md`: "Never gush, never emoji, never exclaim. Warmth is dry, not effusive." A formant synthesizer cannot convey dry warmth — it conveys nothing. The butler register lives in prosody, not just text.
2. **British male voices exist.** `bm_george` and `bm_fable` are immediately available. Grade C-D quality is acceptable for v-first; fine-tuning on a butler-appropriate dataset (public domain British audiobooks) is a post-v1 upgrade.
3. **Decoder-only architecture.** StyleTTS2 is decoder-only — architecturally closer to our Qwen forward than VITS's flow-based generation. This matters if we ever consider ring-0 integration (we don't recommend it, but it's worth noting).
4. **Apache 2.0 license.** No copyleft concerns for distribution.
5. **82M params is small.** Runs on CPU without GPU. PyTorch dependency is the cost, but as a userland process this is contained.

**Why not Piper as primary:** GPL-3.0 is the blocker. If our OS distribution includes Piper, the copyleft obligation applies. It's the smallest and fastest option (ONNX on Pi 4), and would be the right choice if we were distributing under GPL or if the license team clears it. Banked as the **license-contingent alternative**.

**Why not eSpeak-NG:** The butler persona cannot survive formant synthesis. It's a fallback for boot messages or system alerts where quality doesn't matter, not for Arthur's voice.

### Fork statement (for PL ratification)

**TTS Fork: Neural (Kokoro) vs Classical (eSpeak-NG) vs License-contingent (Piper).** Prometheus recommends Kokoro (Apache 2.0, butler-appropriate prosody). If GPL-3.0 is acceptable for the distribution, Piper is smaller and faster. eSpeak-NG is the deterministic fallback but fails the persona test.

---

## Q3 — The substrate-integration call (the OT-16 parallel)

### The three options

| Option | Description | Cost | Fit for voice models |
|--------|------------|------|---------------------|
| **(A) New ring-0 forward** | Add a Whisper-style encoder-decoder or StyleTTS2 forward to `forward_logic.rs` | High — kernel change, new ops (conv/mel frontend for STT; flow/GAN vocoder for TTS), breaks the decoder-only invariant | Poor — voice models are architecturally very different from Qwen |
| **(B) Ride OT-16's data-driven forward graph** | Encode the voice model as a `.neom` schedule for the OT-16 walker | Medium — requires new op kinds (conv, mel-spectrogram, ISTFT) in the walker's dispatch table | Possible in principle but adds voice-specific ops to the kernel walker |
| **(C) Userland inference process** | Separate ring-3 process runs the voice model; communicates with Arthur via IPC or shared memory | Low — no kernel change, clean isolation, standard ML runtime (PyTorch/OnnxRuntime) | **Best fit** — voice models are userland workloads |

### Verdict: Userland inference process (Option C)

**Rationale:**

1. **Architectural mismatch.** Our ring-0 forward is a decoder-only transformer forward (Qwen2: GQA → RoPE → SwiGLU FFN). Whisper-class STT is an **encoder-decoder** with a **conv/mel frontend** (audio → mel-spectrogram → encoder → decoder). Kokoro TTS is **StyleTTS2 + ISTFTNet vocoder** (text → style tokens → decoder → inverse-STFT → waveform). These are fundamentally different architectures requiring different op sets.
2. **OT-16's purpose.** OT-16 generalizes the forward *graph* (layer-kind schedule from `.neom`), not the *op set*. Adding conv, mel-spectrogram, and ISTFT ops to the kernel walker would bloat it with voice-specific operations that have nothing to do with LLM inference. The ANSPEC §19.2 principle — "the op set changes on a decade scale" — argues for keeping the kernel op set minimal.
3. **I-14 fault containment.** Voice models introduce audio devices (mic capture, speaker output) with DMA. Running voice inference in userland isolates audio-DMA faults from the kernel inference path. A mic driver fault cannot stall `SYS_GENERATE_MASKED`.
4. **Precedent.** whisper.cpp is a standalone C++ binary. Moonshine uses OnnxRuntime as a separate process. Piper is a standalone TTS server. These are designed as userland processes, not kernel modules.
5. **No new ring-0 forward needed.** Voice inference doesn't need the kernel's quantized matmul path. It needs standard ML runtimes (OnnxRuntime, PyTorch) that handle their own memory management and threading.

### The OT-16 parallel: why this is different from DeltaNet

DeltaNet (OT-16's original trigger) was a **decoder-only transformer** — same architecture family as Qwen, different layer configuration. It could ride the data-driven forward graph because the op set (matmul, softmax, attention, FFN) was identical. Voice models are a **different architecture class** (encoder-decoder, conv frontend, vocoder) — they don't share the op set. This is not "another model config," it's "a different kind of computation."

### Cost of Option C

- **IPC overhead:** Arthur (userland) sends text to STT/TTS processes via Unix domain socket or shared memory. For push-to-talk, the latency budget is ~200ms (Moonshine Small) + IPC (~1ms negligible). No concern.
- **Memory:** Moonshine Small (~492 MB FP32) + Kokoro (~328 MB + voice tensors) ≈ ~820 MB resident when both are active. Acceptable for a machine running LogansOS (the bare-metal target has more).
- **Process management:** Arthur spawns the STT/TTS processes at boot (or on-demand). They stay resident for fast response. Kill-switch: kill the process, voice is off, Arthur reverts to text-only.

---

## Q4 — Reference availability for the parity oracle

| Model | Correct reference? | Parity oracle viable? |
|-------|-------------------|---------------------|
| **Moonshine** | Yes — portable C++ core library, OnnxRuntime backend | Yes — run the reference C++ lib on a known audio sample, compare output against our integration |
| **Kokoro** | Yes — hexgrad/kokoro repo, PyTorch reference | Yes — run the reference model on a known text input, compare waveform output |
| **Piper** | Yes — OHF-Voice/piper1-gpl, ONNX models | Yes — deterministic ONNX inference is trivially parity-checkable |

**Verdict:** All recommended candidates have correct reference implementations. No candidate is "parked" for lack of a reference (the DeltaNet-Q1 gate that blocked Gemma-4).

---

## Q5 — Privacy architecture + audio-device dimension

### Push-to-talk / local-only mechanism

**Design: Explicit key or gesture, NEVER wake-word.**

1. **Physical key:** A dedicated keyboard key (e.g., a remapped media key) that Arthur listens for. When pressed, the mic captures until release (or until silence detected by VAD). This is the strongest privacy guarantee — no audio processing occurs unless the key is physically pressed.
2. **Explicit gesture:** For touch-screen or future GUI: a visible "talk" button. Same semantics — explicit user action starts capture.
3. **Why not wake-word:** A wake-word detector requires always-listening to the mic, even if only a tiny on-device model. This is the surveillance architecture Article 23.1 forbids. Even a local wake-word means the mic is always active, always DMA-ing audio into memory. Push-to-talk is structurally safer: the mic is silent until the owner activates it.

**Data flow (all on-device):**
```
[Physical key press] → [mic capture starts] → [raw PCM buffer, ring-3]
  → [Moonshine STT process, ring-3] → [transcribed text, IPC]
  → [Arthur agent, ring-3] → [action dispatch] → [result text]
  → [Kokoro TTS process, ring-3] → [waveform PCM, IPC]
  → [speaker output, ring-3]
```

No audio data ever enters the kernel. No audio data ever leaves the machine. The kernel's role is zero — audio devices are userland peripherals.

### I-14 fault-containment vectors (audio-DMA)

Audio devices introduce new DMA vectors that the eventual build must contain:

1. **Mic capture DMA:** The audio input device (USB-audio or HDA/AC97) DMA-transfers PCM samples from hardware into a ring buffer in system memory. **I-14 vector:** a mic driver fault or malicious USB-audio device could write to arbitrary memory via DMA. **Containment:** the DMA arena must be fenced (I-14(a)) — audio DMA buffers are in a dedicated, capability-gated memory region. A fault in the audio path cannot corrupt kernel memory or other process memory.
2. **Speaker output DMA:** Symmetric — PCM waveform DMA from memory to the audio output hardware. Lower risk (output-only) but same fencing discipline applies.
3. **USB-audio device class:** USB audio devices are inherently less trusted than internal HDA/AC97. A malicious USB mic could attempt DMA attacks. The USB controller's IOMMU (if present) must be configured to fence USB audio DMA to its allocated buffers.
4. **Audio process isolation:** The STT/TTS processes run as separate userland tenants. A fault in the Moonshine process (e.g., OnnxRuntime segfault on a malformed model) cannot crash Arthur or the kernel. Morganna guardrail: killable, dismissable.

### Audio device drivers (target hardware)

The OptiPlex / typical laptop presents:
- **HDA/AC97** (internal mic + speakers) — standard Intel HDA controller
- **USB-audio** (headsets, external mics) — USB Class Audio device

For the eventual build, we need:
- An HDA driver (or AC97, depending on the specific OptiPlex model) that exposes a PCM capture/playback interface to userland
- A USB-audio driver for external devices
- Both drivers must respect I-14 DMA fencing

**This is research-level flagging.** The full device-driver build is the Phase-7.5 connectivity lane's job. This dispatch names the vectors; it doesn't build the drivers.

---

## ISC-1 — The STT recommendation

**Model:** Moonshine Small (123M params, ~492 MB FP32)
**On-device cost:** 165ms latency on Linux x86 CPU; 527ms on Raspberry Pi 5. 7.84% WER on short-form OOD. Runs CPU-only via OnnxRuntime.
**Substrate integration:** Userland inference process (Option C). No new ring-0 forward. No OT-16 rider.
**Reference availability:** Yes — portable C++ core library is the correct reference for parity oracle. Moonshine Tiny (34M params, 69ms) is the size-constrained alternative if 12% WER is acceptable for the first slice.

---

## ISC-2 — The TTS recommendation

**Model:** Kokoro (StyleTTS2, 82M params, ~328 MB FP32) with British male voices (`bm_george`, `bm_fable`)
**The real fork:** Neural (Kokoro/Piper) vs Classical (eSpeak-NG). Classical skips the neural substrate entirely but fails the butler persona test — formant synthesis cannot convey dry warmth.
**Butler fit:** Kokoro's StyleTTS2 produces natural prosody with style transfer capability. British male voices are grade C-D (acceptable for v-first; fine-tuning is a post-v1 upgrade).
**License fork:** Kokoro is Apache 2.0. Piper is GPL-3.0 (smaller, faster, but copyleft). PL must decide if GPL-3.0 is acceptable for the distribution.

---

## ISC-3 — The substrate-integration verdict

**Verdict: Userland inference process.** Voice models are architecturally distinct from our decoder-only Qwen forward. They do NOT ride OT-16's data-driven forward graph, and they do NOT need a new ring-0 forward. The cost is ~820 MB resident memory (Moonshine Small + Kokoro) + negligible IPC overhead. Kill-switch: kill the process.

**OT-16 parallel:** DeltaNet rode OT-16 because it was the same architecture family (decoder-only transformer, same op set). Voice models are a different architecture class (encoder-decoder, conv frontend, vocoder) — different ops, different runtime, different isolation boundary.

**PL ratification point:** Confirm userland inference as the integration model. If the PL later decides voice should ride OT-16 (e.g., for tighter latency integration), that's a Phase-N design decision requiring new kernel ops (conv, mel-spectrogram, ISTFT) — a real cost against the invariant budget.

---

## ISC-4 — Privacy + device architecture

**Push-to-talk mechanism:** Physical key or explicit gesture. No wake-word. Mic is silent until activated. VAD (Voice Activity Detection) ends capture on silence after the key is released.
**Local-only:** The entire path — mic → STT → Arthur → TTS → speaker — is on-device. No audio leaves the machine. Reinforces sovereignty, never the surveillance model Article 23.1 forbids.
**I-14 vectors flagged:** (a) Mic DMA fence — audio DMA buffers in dedicated, capability-gated memory region; (b) Speaker DMA fence — symmetric output-only fencing; (c) USB-audio device trust — IOMMU-fenced USB audio DMA; (d) Audio process isolation — STT/TTS as separate killable userland tenants.
**Device drivers:** HDA/AC97 (internal) + USB-audio (external). Build is Phase-7.5's job; this dispatch names the requirement.

---

## ISC-5 — Feasibility verdict + forks

### Feasibility: Yes, at known cost

On-device bidirectional voice is feasible on our substrate now. The components exist, are proven on-device, and have correct references. The integration is clean (userland processes). The privacy architecture is structurally sound (push-to-talk, local-only).

**Smallest viable first slice:** **STT-only.** Moonshine Small gives the five-year-old voice input without needing TTS output. Arthur responds in text on the transcript (the v1 interface per OT-26). This is the fastest path to "boot the OptiPlex and talk to it" for the input half. TTS is the quality-of-life upgrade — it makes the butler speak, but the transcript already carries the response.

**Full bidirectional cost:** ~820 MB resident (Moonshine Small + Kokoro) + PyTorch/OnnxRuntime dependencies + audio device drivers (Phase-7.5).

### Forks for PL ratification

| Fork | Options | Prometheus recommendation |
|------|---------|--------------------------|
| **STT model** | Moonshine Small (recommended) / Moonshine Tiny (smaller, slower accuracy) / Whisper.cpp Base (proven, slower) | Moonshine Small |
| **TTS: neural vs classical** | Kokoro (neural, Apache 2.0) / Piper (neural, GPL-3.0) / eSpeak-NG (classical, robotic) | Kokoro (unless GPL-3.0 is acceptable → Piper) |
| **TTS license** | Apache 2.0 (Kokoro) vs GPL-3.0 (Piper) | PL decision — depends on distribution model |
| **Integration** | Userland process (recommended) / OT-16 rider (high cost, different ops) / New ring-0 forward (highest cost) | Userland process |
| **First slice** | STT-only (fastest to voice input) / TTS-only (voice output only) / Both (full bidirectional) | STT-only first, then add TTS |
| **VAD model** | Silero-VAD (864 KB, Whisper.cpp ecosystem) / Moonshine built-in streaming end-detection | Moonshine built-in (fewer dependencies) |

### Morganna guardrail: invariant

Whatever the substrate, Arthur-with-voice stays a dismissable userland tenant. Killable (`kill <pid>`). Forget-able (`forget` burns memory keys). The `neo>` shell is always the voice-free, Arthur-free path. Voice is a convenience layer over the transcript, not a replacement for it.

---

## Appendix: Honest residuals

1. **Moonshine license:** The LICENSE file at the repo root was referenced but not read firsthand in this dispatch. The README states Apache 2.0; confirm before distribution.
2. **Kokoro British voice quality:** Grade C-D (MM minutes training). Sufficient for v-first evaluation but likely needs fine-tuning for production butler voice. A post-v1 investment.
3. **Audio driver maturity:** HDA/AC97 drivers for LogansOS are unbuilt. The Phase-7.5 connectivity lane must deliver them before voice can be exercised on bare metal. This is the primary build blocker (not the model choice).
4. **PyTorch/OnnxRuntime as userland dependencies:** These are substantial userland libraries. Their inclusion in the LogansOS userland image is a distribution decision (size, license, maintenance). Banked for the OT-22 userland-frontier sequencing.
5. **Piper maintainability:** The OHF-Voice repo notes "seeking maintainers." If Piper is chosen, verify active maintenance before committing.
