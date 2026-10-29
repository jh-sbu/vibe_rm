#!/usr/bin/env python3
"""Offscreen journal smoke checks using an owned vanilla SE installation.
Build first with cargo build. Outputs go to ignored tests/assets by default.
Never writes to the game's Data directory or runs Skyrim itself.
"""
import argparse
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=Path('target/debug/vibe_rm'))
parser.add_argument('--out', type=Path, default=Path('tests/assets/system-page'))
args = parser.parse_args()
binary = args.binary.resolve()
out = args.out.resolve()
out.mkdir(parents=True, exist_ok=True)
saves = out / 'saves'
base = os.environ | {
    'VRM_SAVES': str(saves), 'VRM_AUDIO_SETTINGS': str(out / 'audio.ini'),
    'RUST_LOG': 'warn,vrm_ui_swf=trace',
}

def capture(name, keys, *, console=(), extra=()):
    env = base | {'VRM_SETTINGS': str(out / (name + '.ini')),
                  'VRM_CONTROLS': str(out / (name + '-controls.txt'))}
    command = [str(binary), '--cell', 'WhiterunBanneredMare', '--no-scripts']
    for line in console:
        command += ['--console', line]
    for key in keys:
        command += ['--menu-key', key]
    command += list(extra) + ['--screenshot', str(out / (name + '.png'))]
    with (out / (name + '.log')).open('w') as log:
        subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    assert (out / (name + '.png')).stat().st_size > 100
    print(name, flush=True)
    return (out / (name + '.log')).read_text()

system = ['Escape', 'click:900,70']
# A malformed known value must neither discard unrelated preferences nor cause
# an unchanged journal close to rewrite either INI (including unknown keys).
original_settings = '[GamePlay]\niDifficulty=broken\n[Controls]\nbInvertYValues=1\n[Future]\nexample=kept\n'
original_audio = '[AudioMenu]\nfAudioMasterVolume=.7\n[Future]\nexample=kept\n'
(out / 'unchanged.ini').write_text(original_settings)
(out / 'audio.ini').write_text(original_audio)
log = capture('unchanged', system + ['Escape', 'pause', 'pause', 'pause'])
assert 'menu calls CloseMenu(' in log
assert (out / 'unchanged.ini').read_text() == original_settings
assert (out / 'audio.ini').read_text() == original_audio
assert not (out / 'unchanged-controls.txt').exists()

capture('main', system, console=['save one', 'save two'])
log = capture('load', system + ['click:380,417', 'pause', 'pause', 'pause'])
assert 'PrepSaveGameScreenshot' in log
capture('characters', system + ['click:380,417', 'KeyT'])
capture('save', system + ['click:380,391', 'pause', 'pause', 'pause'])
capture('gameplay', system + ['click:368,468', 'click:477,198'])
capture('display', system + ['click:368,468', 'click:477,234'])
capture('audio', system + ['click:368,468', 'click:477,266'])
capture('controls', system + ['click:368,494'])
capture('remap', system + ['click:368,494', 'click:685,199', 'pause'])
log = capture('remapped', system + ['click:368,494', 'click:685,199', 'pause', 'KeyG', 'Escape', 'pause', 'pause', 'pause'])
assert 'StartRemapMode' in log
assert 'menu calls SaveControls(' in log
assert 'Forward\t0x22\t' in (out / 'remapped-controls.txt').read_text()
capture('help', system + ['click:380,520'])
log = capture('help-topic', system + ['click:380,520', 'click:481,266'])
assert 'GetButtonFromUserEvent' in log
quit_list = system + ['ArrowDown'] * 7 + ['Enter']
capture('quit', quit_list)
capture('quit-confirm', quit_list + ['click:479,372'])
before = set(saves.glob('*.vrs'))
capture('saved', system + ['click:380,391', 'click:502,192', 'pause'])
created = set(saves.glob('*.vrs')) - before
assert len(created) == 1 and next(iter(created)).name.startswith('Save ')
log = capture('loaded', system + ['click:380,417', 'click:502,192', 'Enter', 'pause'])
assert 'menu calls LoadGame(' in log
before = set(saves.glob('*.vrs'))
log = capture('deleted', system + ['click:380,417', 'KeyX', 'Enter', 'pause'])
assert 'menu calls DeleteSave(' in log
assert len(before - set(saves.glob('*.vrs'))) == 1
capture('resume', [], extra=['--load', 'one'])
print(f'Captures and logs: {out}')
