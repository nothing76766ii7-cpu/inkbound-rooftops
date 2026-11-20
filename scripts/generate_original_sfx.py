import numpy as np, wave, os
from scipy.signal import butter, sosfilt
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1] / 'assets' / 'audio'
ROOT.mkdir(parents=True,exist_ok=True)
SR=48000
rng=np.random.default_rng(20261120)

def sound(name, dur, tones=(), noise=0, thud=0, chirp=None, tail=.12, vol=.74):
    n=int(dur*SR); t=np.arange(n)/SR
    y=np.zeros(n)
    attack=1-np.exp(-t*140)
    decay=np.exp(-t/max(.03,dur*0.32))
    for hz,amp,sweep in tones:
        phase=2*np.pi*(hz*t+0.5*sweep*t*t)
        y+=amp*(np.sin(phase)+0.19*np.sin(2*phase)) * np.exp(-t/max(.03,dur*0.34))
    if noise:
        sig=rng.standard_normal(n)
        sos=butter(2,[190,6000],btype='bandpass',fs=SR,output='sos')
        y+=noise*sosfilt(sos,sig)*np.exp(-t/max(.02,dur*.27))
    if thud:
        y+=thud*np.sin(2*np.pi*(82*t-20*t*t)) * np.exp(-t/0.092)
    if chirp:
        f0,f1=chirp
        y+=0.4*np.sin(2*np.pi*(f0*t+(f1-f0)*t*t/(2*dur))) * np.exp(-t/max(.04,dur*.38))
    y*=attack
    if tail: y*=np.minimum(1,np.maximum(0,(dur-t)/tail))
    y=np.tanh(y*1.4)
    y/=max(1.,np.max(np.abs(y))/.90)
    pcm=(y*vol*32767).astype('<i2')
    with wave.open(str(ROOT/f'{name}.wav'),'wb') as f:
        f.setnchannels(1); f.setsampwidth(2); f.setframerate(SR); f.writeframes(pcm.tobytes())

sound('footstep_roof',.16,[(115,.32,-360)],.35,.25)
sound('jump',.24,[(210,.24,340)],.18,.14,chirp=(180,380))
sound('wall_jump',.32,[(260,.3,380)],.24,.17,chirp=(210,620))
sound('landing_hard',.49,[(98,.31,-100)],.72,.88)
sound('dash',.32,[(300,.13,-500)],.82,.09,chirp=(950,170))
sound('sword_light',.34,[(270,.19,-320)],.89,.05,chirp=(1500,330))
sound('sword_heavy',.62,[(150,.4,-95),(61,.25,-15)],.75,.5,chirp=(880,70))
sound('dimensional_rift',1.18,[(175,.48,-55),(92,.32,26),(280,.23,-80)],.35,.8,chirp=(760,92))
sound('slash_impact',.56,[(124,.24,-185),(190,.11,-150)],.85,.75)
sound('paint_stroke',.46,[(430,.26,240),(670,.09,210)],.30,.06,chirp=(520,850))
sound('paint_platform',.74,[(350,.32,-150),(525,.26,-90),(700,.11,-65)],.30,.17)
sound('glyph_activated',1.08,[(440,.3,10),(554,.3,10),(660,.2,10)],.12,.04,tail=.20)
sound('ui_tick',.085,[(790,.27,-2100)],.06,0,tail=.045)
sound('hook_fire',.52,[(240,.22,-180),(550,.12,-300)],.44,.07,chirp=(280,950))
sound('hook_attach',.46,[(315,.32,-90),(630,.16,-140)],.35,.38,chirp=(940,330))
sound('hook_release',.23,[(280,.29,-330)],.24,.13,chirp=(390,100))
sound('hook_miss',.24,[(190,.20,-280)],.11,0)
sound('hook_reel',.19,[(200,.18,250),(420,.10,110)],.14,.07)
sound('stone_crack',.61,[(90,.18,-45)],.90,.75)
sound('debris_fall',.92,[(55,.19,-15)],.75,.69)
print(len(list(ROOT.glob('*.wav'))),sum(p.stat().st_size for p in ROOT.iterdir()))
