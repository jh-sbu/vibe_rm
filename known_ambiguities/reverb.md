# Reverb approximation

Sources: [UESP REVB revision 3089102](https://en.uesp.net/w/index.php?oldid=3089102),
[SOPM revision 3089107](https://en.uesp.net/w/index.php?oldid=3089107), game assets,
and [Microsoft I3DL2 parameter documentation](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2fx/ns-xaudio2fx-xaudio2fx_reverb_i3dl2_parameters).

We interpret signed byte levels as dB (I3DL2's hundredths of dB divided by
100), decay in ms, HF ratio /100, scaled reflection delay as byte*300/249 ms,
and the reverb delay relative to the early reflection. -100 dB room bypasses
processing. NAM1 byte 3 is a percentage send; 2D voices without a model send
nothing. These field mappings fit the data, but the delay scaling is only
approximately documented.

The original eight-delay Householder feedback network uses room/HF input
filtering, early and late delays, per-band decay, diffusion feedback mixing,
and density-dependent delay lengths. It approximates the parameter meanings,
not XAudio2's response. No third-party DSP source was copied. Space changes
crossfade two networks over 1.5 seconds; rapid changes replace the older tail.
Memory is allocated/reclaimed outside the callback, which processes at most
two networks. Underwater selection is the choice in water.md. Listening is
needed to compare the approximation's tone and decay with SE.
