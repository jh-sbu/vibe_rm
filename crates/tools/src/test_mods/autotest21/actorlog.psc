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

; Added for autotest29: blows counted as they land, and the blow at which
; a watched disease first shows (HasSpell, HasMagicEffect, max stamina
; below a mark). A hit whose source is a spell is counted apart.
Int Blows = 0
Int SpellHits = 0
Int LastSpellSource = 0
Spell WatchSpell
MagicEffect WatchEffect
Float WatchStamina = 0.0
Int SpellBlow = -1
Int EffectBlow = -1
Int StaminaBlow = -1

Function Watch(Spell s, MagicEffect e, Float staminaMark)
    Blows = 0
    SpellHits = 0
    LastSpellSource = 0
    SpellBlow = -1
    EffectBlow = -1
    StaminaBlow = -1
    WatchSpell = s
    WatchEffect = e
    WatchStamina = staminaMark
EndFunction

; Checks the watched disease after a hit; the blow so far is the one.
Function CheckWatch()
    If WatchStamina <= 0.0
        return
    EndIf
    If SpellBlow < 0 && HasSpell(WatchSpell)
        SpellBlow = Blows
    EndIf
    If EffectBlow < 0 && HasMagicEffect(WatchEffect)
        EffectBlow = Blows
    EndIf
    If StaminaBlow < 0 && GetActorValueMax("Stamina") < WatchStamina
        StaminaBlow = Blows
    EndIf
EndFunction

String Function Counts()
    return Blows + " blows, " + SpellHits + " spell hits (last #" + LastSpellSource + "); first at blow: HasSpell " + SpellBlow + ", HasMagicEffect " + EffectBlow + ", max stamina down " + StaminaBlow
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
    Spell sp = akSource as Spell
    If sp != None
        SpellHits += 1
        LastSpellSource = sp.GetFormID()
    Else
        Blows += 1
    EndIf
    CheckWatch()
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
