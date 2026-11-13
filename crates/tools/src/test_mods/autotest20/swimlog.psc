Scriptname VrmAt20SwimLog extends ActiveMagicEffect
; autotest20's detector: the swimming ability's two effects (IsSwimming 1
; and 0) report starting and finishing to the test quest.

VrmAt20Script Property Owner Auto
String Property Kind Auto

Event OnEffectStart(Actor akTarget, Actor akCaster)
    Owner.Note(akTarget, Kind, true)
EndEvent

Event OnEffectFinish(Actor akTarget, Actor akCaster)
    Owner.Note(akTarget, Kind, false)
EndEvent
