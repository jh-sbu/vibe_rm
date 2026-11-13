Scriptname VrmAt25SwimLog extends ActiveMagicEffect
; autotest25's detector (autotest20's): the swimming ability's two effects
; (IsSwimming 1 and 0) report starting and finishing to the test quest.

VrmAt25Script Property Owner Auto
String Property Kind Auto

Event OnEffectStart(Actor akTarget, Actor akCaster)
    Owner.Note(akTarget, Kind, true)
EndEvent

Event OnEffectFinish(Actor akTarget, Actor akCaster)
    Owner.Note(akTarget, Kind, false)
EndEvent
