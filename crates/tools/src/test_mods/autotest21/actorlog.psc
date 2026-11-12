Scriptname VrmAt21ActorLog extends Actor
; Logs what happens to the actor it is attached to (every actor of a base
; carrying it), each event with the real time in hundredths.
String Log = ""
Int Marker = 7

Function Add(String s)
    Log = Log + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + " "
EndFunction

String Function Take()
    String s = Log
    Log = ""
    return s
EndFunction

Event OnHit(ObjectReference akAggressor, Form akSource, Projectile akProjectile, Bool abPowerAttack, Bool abSneakAttack, Bool abBashAttack, Bool abHitBlocked)
    String s = "hit"
    If abPowerAttack
        s += "+power"
    EndIf
    If abSneakAttack
        s += "+sneak"
    EndIf
    If abBashAttack
        s += "+bash"
    EndIf
    If abHitBlocked
        s += "+blocked"
    EndIf
    ; Added for test-mod explosions: the source's and aggressor's ids.
    If akSource != None
        s += "#" + akSource.GetFormID()
    EndIf
    If akAggressor != None
        s += "<" + akAggressor.GetFormID()
    EndIf
    Add(s)
EndEvent

Event OnCombatStateChanged(Actor akTarget, Int aeCombatState)
    Add("combat" + aeCombatState)
EndEvent

Event OnEnterBleedout()
    Add("bleedout")
EndEvent

Event OnDying(Actor akKiller)
    Add("dying")
EndEvent

Event OnDeath(Actor akKiller)
    Add("death")
EndEvent

Event OnSpellCast(Form akSpell)
    If akSpell != None
        Add("cast#" + akSpell.GetFormID())
    Else
        Add("cast")
    EndIf
EndEvent

Event OnMagicEffectApply(ObjectReference akCaster, MagicEffect akEffect)
    Add("effect")
EndEvent

Event OnPackageStart(Package akNewPackage)
    Add("pkgstart")
EndEvent

Event OnPackageEnd(Package akOldPackage)
    Add("pkgend")
EndEvent

Event OnPackageChange(Package akOldPackage)
    Add("pkgchange")
EndEvent

Event OnObjectEquipped(Form akBaseObject, ObjectReference akReference)
    Add("equip")
EndEvent

Event OnObjectUnequipped(Form akBaseObject, ObjectReference akReference)
    Add("unequip")
EndEvent

Event OnSit(ObjectReference akFurniture)
    Add("sit")
EndEvent

Event OnGetUp(ObjectReference akFurniture)
    Add("getup")
EndEvent

Event OnAnimationEvent(ObjectReference akSource, String asEventName)
    Add(asEventName)
EndEvent

Event OnLoad()
    Add("load")
EndEvent

Event OnUnload()
    Add("unload")
EndEvent

Event OnWardHit(ObjectReference akCaster, Spell akSpell, Int aiStatus)
    Add("ward" + aiStatus)
EndEvent

Event OnLocationChange(Location akOldLoc, Location akNewLoc)
    Add("location")
EndEvent
