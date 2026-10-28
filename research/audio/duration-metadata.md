# Header-only audio timing (2026-10-27)

The local UESP index had no XWMA or Skyrim Mod WAV format pages. Primary public
references checked next:

- [Microsoft RIFF description](https://learn.microsoft.com/en-us/windows/win32/xaudio2/resource-interchange-file-format--riff-): tagged chunks, byte lengths and WORD padding, WAVE/XWMA form types.
- [WAVEFORMATEX](https://learn.microsoft.com/en-us/windows/win32/api/mmeapi/ns-mmeapi-waveformatex): per-channel sample rate and PCM frame/block alignment; compressed block alignment is codec-specific.
- [XAUDIO2_BUFFER_WMA](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer_wma): dpds is an array of cumulative decoded byte counts.

Read-only asset checks found 269 music XWM files in Skyrim - Sounds.bsa.
Three sampled files (bgssse016_mus_combat_boss_01, mus_combat_01 and its
finale) all have XWMA form type, WMAudio2 tag 0x161, two channels, 44100 Hz,
2230 encoded bytes/block and 16 bits/sample. Their last dpds decoded-byte
counts are 10682368, 18620416 and 2195456. Assets were inspected in temporary
files outside the repository and removed; none is committed or placed in refs/.

Duration is PCM data bytes / frame alignment / sample rate, or XWMA's last
dpds value / decoded bytes per frame / sample rate. Encoded block alignment
and average encoded bitrate are not used as decoded frame sizes. FUZE's lip
payload is skipped before inspecting its contained RIFF audio. The parser
validates bounds, metadata divisibility and monotonic packet counts; it does
not guess duration for missing/unsupported metadata or fall back to decoding.
No third-party implementation code was incorporated.
